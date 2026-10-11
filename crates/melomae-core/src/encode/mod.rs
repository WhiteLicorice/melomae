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
    /// The encoder rejected the input or a parameter.
    Message(String),
}

impl fmt::Display for EncodeFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Message(reason) => f.write_str(reason),
        }
    }
}

impl From<std::io::Error> for EncodeFailure {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<String> for EncodeFailure {
    fn from(reason: String) -> Self {
        Self::Message(reason)
    }
}

/// One encoder's result, before the caller adds the target path.
type WriteResult = Result<EncodeOutcome, EncodeFailure>;

/// Encode `source` to `target` in the given format.
///
/// The encoder writes a temporary file in the target directory and renames it to `target` only
/// when the stream is complete. An incomplete stream never reaches `target`, and the temporary
/// file is removed on any error.
pub fn encode(
    source: &AudioBuffer,
    target: &Path,
    format: OutputFormat,
) -> Result<EncodeOutcome, EncodeError> {
    if let Some(parent) = target.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| {
                EncodeError::new(target, format!("create the output directory: {e}"))
            })?;
        }
    }

    let temporary = temporary_path(target);
    let outcome = match write_stream(&temporary, source, format) {
        Ok(outcome) => outcome,
        Err(reason) => {
            let _ = fs::remove_file(&temporary);
            return Err(EncodeError::new(target, reason.to_string()));
        }
    };

    if let Err(error) = fs::rename(&temporary, target) {
        let _ = fs::remove_file(&temporary);
        return Err(EncodeError::new(
            target,
            format!("rename the completed file into place: {error}"),
        ));
    }
    Ok(outcome)
}

/// Write one encoded stream to `path`, which must be a temporary file.
fn write_stream(path: &Path, source: &AudioBuffer, format: OutputFormat) -> WriteResult {
    let file = fs::File::create(path)?;
    let mut writer = BufWriter::new(file);
    let outcome = match format {
        OutputFormat::Flac => flac::write(&mut writer, source),
        OutputFormat::Wav => wav::write(&mut writer, source),
        OutputFormat::Opus => opus::write(&mut writer, source),
    }?;
    writer.flush()?;
    Ok(outcome)
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
}
