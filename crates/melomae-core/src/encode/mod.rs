//! Encode restored audio to FLAC, WAV, or Opus.
//!
//! [`encode`] writes one [`AudioBuffer`] to a file. It picks the writer for the output format,
//! writes the stream to a temporary file next to the target, and renames the file into place only
//! when the stream is complete. FLAC and WAV keep the sample rate of the input; Opus resamples to
//! 48 kHz, which libopus always codes.
//!
//! FLAC uses the pure-Rust `flacenc` encoder at 24 bits. WAV uses the IEEE float 32-bit form, so
//! it is bit-exact. Opus uses the vendored libopus from `opusic-c` and a small Ogg writer.

mod flac;
mod ogg;
mod opus;
mod wav;

use std::fmt;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::decode::AudioBuffer;

/// The output format of a restore.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// FLAC, 24-bit, at the input sample rate. This is the default.
    Flac,
    /// WAV, IEEE float 32-bit, at the input sample rate.
    Wav,
    /// Opus in Ogg, 48 kHz, 256 kb/s VBR.
    Opus,
}

impl OutputFormat {
    /// The file-name extension for the format, without the dot.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Flac => "flac",
            Self::Wav => "wav",
            Self::Opus => "opus",
        }
    }

    /// Whether the format stores integers, and therefore clamps samples outside the +/-1.0 range.
    pub fn is_integer(self) -> bool {
        matches!(self, Self::Flac)
    }
}

/// The result of one encode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EncodeOutcome {
    /// The number of samples clamped to the +/-1.0 range. WAV and Opus clamp no sample, so this is
    /// zero for those formats.
    pub clamped: usize,
}

/// The result of an encode that a caller can cancel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeResult {
    /// The stream completed and was renamed into place.
    Complete(EncodeOutcome),
    /// The caller cancelled before the final rename. Nothing was published and no temporary file
    /// remains.
    Cancelled,
}

/// An error from encoding an output file.
///
/// The error always names the file and states the reason, so a caller can report both.
#[derive(Debug)]
pub struct EncodeError {
    path: PathBuf,
    reason: String,
}

impl EncodeError {
    fn new(path: &Path, reason: impl Into<String>) -> Self {
        Self {
            path: path.to_path_buf(),
            reason: reason.into(),
        }
    }

    /// The file that could not be encoded.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Why the file could not be encoded.
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.reason)
    }
}

impl std::error::Error for EncodeError {}

/// The reason an encoder failed. The caller adds the target path.
///
/// It stays internal: the public [`EncodeError`] carries the path and a rendered reason.
#[derive(Debug)]
enum EncodeFailure {
    /// The output file could not be written.
    Io(std::io::Error),
    /// The caller cancelled while the stream was being written.
    Cancelled,
    /// The encoder rejected the input or a parameter.
    Message(String),
}

impl fmt::Display for EncodeFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Cancelled => f.write_str("the caller cancelled the encode"),
            Self::Message(reason) => f.write_str(reason),
        }
    }
}

impl From<std::io::Error> for EncodeFailure {
    fn from(error: std::io::Error) -> Self {
        // The cancellable writer reports a cancel as a broken pipe. `write_all` retries an
        // interrupted write, so the cancel must not use `ErrorKind::Interrupted`.
        if error.kind() == std::io::ErrorKind::BrokenPipe {
            Self::Cancelled
        } else {
            Self::Io(error)
        }
    }
}

impl From<String> for EncodeFailure {
    fn from(reason: String) -> Self {
        Self::Message(reason)
    }
}

/// One encoder's result, before the caller adds the target path.
type WriteResult = Result<EncodeOutcome, EncodeFailure>;

/// The one sample rate the output formats are defined at. Every input is checked against it.
pub const INPUT_SAMPLE_RATE: u32 = 44_100;

/// A flag that is never set, for callers that cannot cancel.
static NEVER_CANCELLED: AtomicBool = AtomicBool::new(false);

/// Encode `source` to `target` in the given format.
///
/// The input must hold at least one frame and one channel, must be at [`INPUT_SAMPLE_RATE`], and
/// must hold only finite samples. A rejected input returns an [`EncodeError`] before the output
/// directory is created or the temporary file exists, so it never touches the file system.
///
/// The encoder writes a temporary file in the target directory and renames it to `target` only
/// when the stream is complete. An incomplete stream never reaches `target`, and the temporary
/// file is removed on any error.
pub fn encode(
    source: &AudioBuffer,
    target: &Path,
    format: OutputFormat,
) -> Result<EncodeOutcome, EncodeError> {
    match encode_with_cancel(source, target, format, &NEVER_CANCELLED)? {
        EncodeResult::Complete(outcome) => Ok(outcome),
        EncodeResult::Cancelled => unreachable!("the never-cancelled flag cannot be set"),
    }
}

/// Encode `source` to `target`, checking `cancel` while the stream is written and once more before
/// the final rename.
///
/// The final rename is the completion boundary. A cancel before it returns
/// [`EncodeResult::Cancelled`], removes the temporary file, and leaves `target` untouched. A cancel
/// after it is ignored, so a completed encode can report success. `target` is never written
/// directly, so an existing file at `target` is only replaced by a completed encode.
pub fn encode_with_cancel(
    source: &AudioBuffer,
    target: &Path,
    format: OutputFormat,
    cancel: &AtomicBool,
) -> Result<EncodeResult, EncodeError> {
    validate(source).map_err(|reason| EncodeError::new(target, reason))?;

    if let Some(parent) = target.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| {
                EncodeError::new(target, format!("create the output directory: {e}"))
            })?;
        }
    }

    if cancel.load(Ordering::Relaxed) {
        return Ok(EncodeResult::Cancelled);
    }

    let temporary = temporary_path(target);
    let outcome = match write_stream(&temporary, source, format, cancel) {
        Ok(outcome) => outcome,
        Err(EncodeFailure::Cancelled) => {
            let _ = fs::remove_file(&temporary);
            return Ok(EncodeResult::Cancelled);
        }
        Err(reason) => {
            let _ = fs::remove_file(&temporary);
            return Err(EncodeError::new(target, reason.to_string()));
        }
    };

    if cancel.load(Ordering::Relaxed) {
        let _ = fs::remove_file(&temporary);
        return Ok(EncodeResult::Cancelled);
    }

    if let Err(error) = fs::rename(&temporary, target) {
        let _ = fs::remove_file(&temporary);
        return Err(EncodeError::new(
            target,
            format!("rename the completed file into place: {error}"),
        ));
    }
    Ok(EncodeResult::Complete(outcome))
}

/// Check the input against the output contract, and return the reason for the first problem.
///
/// A non-finite sample means a failed numerical result upstream. It must not be written as
/// silence, so it is rejected instead of replaced.
fn validate(source: &AudioBuffer) -> Result<(), String> {
    if source.frames() == 0 {
        return Err("the input holds no frames".to_string());
    }
    if source.channel_count() == 0 {
        return Err("the input holds no channels".to_string());
    }
    if source.sample_rate() != INPUT_SAMPLE_RATE {
        return Err(format!(
            "the input sample rate is {} Hz, not {INPUT_SAMPLE_RATE} Hz",
            source.sample_rate()
        ));
    }
    for (channel, plane) in source.channels().iter().enumerate() {
        for (frame, sample) in plane.iter().enumerate() {
            if !sample.is_finite() {
                return Err(format!(
                    "the input holds a non-finite sample at channel {channel} frame {frame}: {sample}"
                ));
            }
        }
    }
    Ok(())
}

/// Write one encoded stream to `path`, which must be a temporary file.
///
/// The stream is written through a [`CancellableWriter`], so a cancel is reported at every write
/// and at the final flush, before the caller renames the file into place.
fn write_stream(
    path: &Path,
    source: &AudioBuffer,
    format: OutputFormat,
    cancel: &AtomicBool,
) -> WriteResult {
    let file = fs::File::create(path)?;
    let mut writer = BufWriter::new(CancellableWriter::new(file, cancel));
    let outcome = match format {
        OutputFormat::Flac => flac::write(&mut writer, source),
        OutputFormat::Wav => wav::write(&mut writer, source),
        OutputFormat::Opus => opus::write(&mut writer, source),
    }?;
    writer.flush()?;
    Ok(outcome)
}

/// A writer that reports a broken pipe once the caller cancels.
///
/// Every encoder writes through a `dyn Write`, so wrapping the file here checks the cancel flag at
/// each write and at the flush, without changing the writers.
struct CancellableWriter<'a, W: Write> {
    inner: W,
    cancel: &'a AtomicBool,
}

impl<'a, W: Write> CancellableWriter<'a, W> {
    fn new(inner: W, cancel: &'a AtomicBool) -> Self {
        Self { inner, cancel }
    }

    /// Return the cancellation error when the caller has set the flag.
    ///
    /// The kind is [`std::io::ErrorKind::BrokenPipe`], not `Interrupted`: `write_all` retries an
    /// interrupted write and would spin forever.
    fn check(&self) -> std::io::Result<()> {
        if self.cancel.load(Ordering::Relaxed) {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "the caller cancelled the encode",
            ))
        } else {
            Ok(())
        }
    }
}

impl<W: Write> Write for CancellableWriter<'_, W> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.check()?;
        self.inner.write(buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.check()?;
        self.inner.flush()
    }
}

/// A hidden temporary path in the target's directory, so the rename stays on one file system.
fn temporary_path(target: &Path) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("output");
    target.with_file_name(format!(
        ".{name}.melomae-{}-{stamp}.tmp",
        std::process::id()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_output_format_reports_its_extension_and_clamping() {
        assert_eq!(OutputFormat::Flac.extension(), "flac");
        assert_eq!(OutputFormat::Wav.extension(), "wav");
        assert_eq!(OutputFormat::Opus.extension(), "opus");
        assert!(OutputFormat::Flac.is_integer());
        assert!(!OutputFormat::Wav.is_integer());
        assert!(!OutputFormat::Opus.is_integer());
    }

    #[test]
    fn the_temporary_path_is_a_hidden_sibling_of_the_target() {
        let temporary = temporary_path(Path::new("/music/out/restored.flac"));
        assert_eq!(temporary.parent(), Some(Path::new("/music/out")));
        let name = temporary.file_name().unwrap().to_str().unwrap();
        assert!(name.starts_with(".restored.flac"), "{name}");
        assert!(name.ends_with(".tmp"), "{name}");
    }

    #[test]
    fn the_input_check_rejects_empty_wrong_rate_and_non_finite_inputs() {
        let nan = AudioBuffer::new(44_100, vec![vec![0.0, f32::NAN, 0.0]]);
        let reason = validate(&nan).expect_err("a NaN sample must be rejected");
        assert!(reason.contains("channel 0 frame 1"), "{reason}");

        assert!(validate(&AudioBuffer::new(44_100, vec![Vec::new()])).is_err());
        assert!(validate(&AudioBuffer::new(0, vec![vec![0.0]])).is_err());
        assert!(validate(&AudioBuffer::new(48_000, vec![vec![0.0]])).is_err());
        assert!(validate(&AudioBuffer::new(44_100, vec![vec![0.0; 4]])).is_ok());
    }
}
