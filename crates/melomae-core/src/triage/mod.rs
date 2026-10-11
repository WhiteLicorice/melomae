//! Triage: which input files can gain from restoration (SRS §4).
//!
//! [`triage_file`] reads one file, measures its spectral cutoff, and assigns one [`Verdict`].
//! [`scan_roots`] walks files and folders and writes the results to the store. A rescan reads only
//! the files whose size or modification time changed, and removes the rows of files that are gone.
//!
//! Triage opens every input read-only. It never writes an input file.
//!
//! The verdict comes from the container and the spectral cutoff. These are estimates; the app says
//! so (SRS §4.4).

mod cutoff;

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use symphonia::core::codecs::audio::well_known::*;
use symphonia::core::codecs::audio::{AudioCodecId, AudioCodecParameters};
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

use crate::decode::decode_file;
use crate::store::{Store, StoreError, TriageItem, Verdict};

pub use cutoff::{measure_cutoff_hz, measure_cutoff_hz_at};

/// A lossy source whose cutoff is below this value gets `restore`. A lossy source at or above it
/// gets `low-benefit` (SRS §4.3).
///
/// Measured, from the task 16 calibration on `fixtures/ori_wav.wav`, at the 80 dB threshold. The
/// value sits between the 192 kb/s MP3 (18605 Hz) and the high-cutoff lossy files (the 320 kb/s MP3
/// at 20015 Hz). The task `Outcome` records the table.
pub const HIGH_BITRATE_CUTOFF_HZ: f64 = 19_500.0;

/// A lossless container whose cutoff is below this value gets `suspect-lossless`. At or above it,
/// the container is `lossless` (SRS §4.3).
///
/// Measured, from the task 16 calibration. The value sits between the FLAC transcoded from the
/// 128 kb/s MP3 (16349 Hz) and the full-band source (22050 Hz). The task `Outcome` records the table.
pub const SUSPECT_LOSSLESS_CUTOFF_HZ: f64 = 19_500.0;

/// The file extensions a folder walk searches for (SRS §4.1).
pub const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "m4a", "wav", "aiff", "aif", "flac", "opus", "ogg", "webm",
];

/// A failure from triage.
#[derive(Debug)]
pub enum TriageError {
    /// A store call failed.
    Store(StoreError),
    /// A file could not be read.
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for TriageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TriageError::Store(error) => write!(formatter, "{error}"),
            TriageError::Io { path, source } => {
                write!(formatter, "{}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for TriageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TriageError::Store(error) => Some(error),
            TriageError::Io { source, .. } => Some(source),
        }
    }
}

impl From<StoreError> for TriageError {
    fn from(error: StoreError) -> Self {
        TriageError::Store(error)
    }
}

/// What one scan did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScanReport {
    /// The files read and triaged in this scan.
    pub scanned: usize,
    /// The files whose size and modification time were unchanged, so they were not read.
    pub skipped: usize,
    /// The stored rows removed because their file no longer exists.
    pub removed: usize,
    /// Of `scanned`, the files that did not decode.
    pub unreadable: usize,
}

/// Triage one file and return its row. The file is opened read-only.
pub fn triage_file(path: &Path) -> Result<TriageItem, TriageError> {
    let metadata = fs::metadata(path).map_err(|source| TriageError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let size_bytes = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
    let modified_at = modified_at_nanos(&metadata);
    let scanned_at = now_unix();
    let path_string = path.to_string_lossy().into_owned();

    let source = probe_source(path);

    Ok(match decode_file(path) {
        Ok(audio) => {
            let spectral_cutoff_hz = measure_cutoff_hz(&audio);
            let duration_seconds = duration_seconds(&audio);
            TriageItem {
                path: path_string,
                size_bytes,
                modified_at,
                container: source.as_ref().and_then(|s| s.container.clone()),
                codec: source.as_ref().and_then(|s| s.codec.clone()),
                bitrate_kbps: bitrate_kbps(size_bytes, duration_seconds),
                sample_rate_hz: source
                    .as_ref()
                    .and_then(|s| s.sample_rate_hz)
                    .or_else(|| Some(i64::from(audio.sample_rate()))),
                bit_depth: source.as_ref().and_then(|s| s.bit_depth),
                duration_seconds,
                spectral_cutoff_hz: Some(spectral_cutoff_hz),
                verdict: verdict_for(
                    source.as_ref().and_then(|s| s.lossless),
                    Some(spectral_cutoff_hz),
                ),
                reason: None,
                scanned_at,
            }
        }
        Err(error) => TriageItem {
            path: path_string,
            size_bytes,
            modified_at,
            container: source.as_ref().and_then(|s| s.container.clone()),
            codec: source.as_ref().and_then(|s| s.codec.clone()),
            bitrate_kbps: None,
            sample_rate_hz: source.as_ref().and_then(|s| s.sample_rate_hz),
            bit_depth: source.as_ref().and_then(|s| s.bit_depth),
            duration_seconds: None,
            spectral_cutoff_hz: None,
            verdict: Verdict::Unreadable,
            reason: Some(error.reason().to_string()),
            scanned_at,
        },
    })
}

/// Walk `roots` and triage every audio file into `store`.
///
/// A root that is a file is triaged whatever its extension. A root that is a folder is searched
/// recursively for [`AUDIO_EXTENSIONS`]. A file is read only when its size or modification time
/// differs from the stored row. A stored row whose file is absent from `roots` is removed.
pub fn scan_roots(store: &Store, roots: &[PathBuf]) -> Result<ScanReport, TriageError> {
    let present = collect_files(roots)?;
    let stored = store.triage_items()?;
    let stored_by_path: HashMap<&str, &TriageItem> = stored
        .iter()
        .map(|item| (item.path.as_str(), item))
        .collect();

    let mut report = ScanReport::default();
    for path in &present {
        let metadata = fs::metadata(path).map_err(|source| TriageError::Io {
            path: path.clone(),
            source,
        })?;
        let size_bytes = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
        let modified_at = modified_at_nanos(&metadata);
        let key = path.to_string_lossy();

        if let Some(existing) = stored_by_path.get(key.as_ref()) {
            if existing.size_bytes == size_bytes && existing.modified_at == modified_at {
                report.skipped += 1;
                continue;
            }
        }

        let item = triage_file(path)?;
        if item.verdict == Verdict::Unreadable {
            report.unreadable += 1;
        }
        store.upsert_triage_item(&item)?;
        report.scanned += 1;
    }

    let present_set: HashSet<String> = present
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    for item in &stored {
        if present_set.contains(&item.path) {
            continue;
        }
        if roots
            .iter()
            .any(|root| Path::new(&item.path).starts_with(root))
        {
            store.delete_triage_item(&item.path)?;
            report.removed += 1;
        }
    }

    Ok(report)
}

/// The container and codec facts triage reads without decoding the audio.
struct SourceMetadata {
    container: Option<String>,
    codec: Option<String>,
    sample_rate_hz: Option<i64>,
    bit_depth: Option<i64>,
    /// `Some(true)` for a lossless codec, `Some(false)` for a lossy codec, `None` when unknown.
    lossless: Option<bool>,
}

/// Probe a file for its container, codec, sample rate, and bit depth.
///
/// The probe reads headers only. It returns `None` when the container is unknown or the file has no
/// audio track.
fn probe_source(path: &Path) -> Option<SourceMetadata> {
    let file = File::open(path).ok()?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(extension);
    }
    let format = symphonia::default::get_probe()
        .probe(
            &hint,
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .ok()?;
    let container = format.format_info().short_name.to_string();
    let track = format.default_track(TrackType::Audio)?;
    let params = track.codec_params.as_ref()?.audio()?;

    Some(SourceMetadata {
        container: Some(container),
        codec: codec_name(params.codec).map(str::to_string),
        sample_rate_hz: params.sample_rate.map(i64::from),
        bit_depth: bit_depth(params),
        lossless: is_lossless(params.codec, path),
    })
}

/// The verdict for a source, given whether its container is lossless and its measured cutoff.
fn verdict_for(lossless: Option<bool>, cutoff_hz: Option<f64>) -> Verdict {
    let Some(cutoff) = cutoff_hz else {
        return Verdict::Unreadable;
    };
    if lossless == Some(true) {
        if cutoff < SUSPECT_LOSSLESS_CUTOFF_HZ {
            Verdict::SuspectLossless
        } else {
            Verdict::Lossless
        }
    } else if cutoff < HIGH_BITRATE_CUTOFF_HZ {
        Verdict::Restore
    } else {
        Verdict::LowBenefit
    }
}

/// The short name of a known codec id.
fn codec_name(id: AudioCodecId) -> Option<&'static str> {
    Some(match id {
        CODEC_ID_MP1 => "mp1",
        CODEC_ID_MP2 => "mp2",
        CODEC_ID_MP3 => "mp3",
        CODEC_ID_AAC => "aac",
        CODEC_ID_FLAC => "flac",
        CODEC_ID_ALAC => "alac",
        CODEC_ID_VORBIS => "vorbis",
        CODEC_ID_OPUS => "opus",
        CODEC_ID_AC3 => "ac3",
        CODEC_ID_EAC3 => "eac3",
        CODEC_ID_WAVPACK => "wavpack",
        CODEC_ID_MONKEYS_AUDIO => "ape",
        CODEC_ID_TRUEHD => "truehd",
        CODEC_ID_TTA => "tta",
        CODEC_ID_RALF => "ralf",
        _ if is_pcm(id) => "pcm",
        _ => return None,
    })
}

/// Whether a codec id is uncompressed PCM.
fn is_pcm(id: AudioCodecId) -> bool {
    id >= CODEC_ID_PCM_S32LE && id <= CODEC_ID_PCM_MULAW
}

/// Whether a codec id is lossless (`Some(true)`), lossy (`Some(false)`), or unknown (`None`).
fn codec_kind(id: AudioCodecId) -> Option<bool> {
    if is_pcm(id)
        || matches!(
            id,
            CODEC_ID_FLAC
                | CODEC_ID_ALAC
                | CODEC_ID_WAVPACK
                | CODEC_ID_MONKEYS_AUDIO
                | CODEC_ID_TRUEHD
                | CODEC_ID_TTA
                | CODEC_ID_RALF
        )
    {
        return Some(true);
    }
    if matches!(
        id,
        CODEC_ID_MP1 | CODEC_ID_MP2 | CODEC_ID_MP3 | CODEC_ID_AAC | CODEC_ID_VORBIS | CODEC_ID_OPUS
    ) {
        return Some(false);
    }
    None
}

/// Whether a codec is lossless (`Some(true)`), lossy (`Some(false)`), or unknown (`None`).
///
/// A codec outside the known sets falls back to the file extension. `.m4a` is ambiguous, so an
/// unknown `.m4a` codec counts as lossy.
fn is_lossless(id: AudioCodecId, path: &Path) -> Option<bool> {
    codec_kind(id).or_else(|| {
        match path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("flac" | "wav" | "aiff" | "aif") => Some(true),
            Some("mp3" | "m4a" | "ogg" | "opus" | "webm") => Some(false),
            _ => None,
        }
    })
}

/// The bit depth of a lossless source, when the container or the codec states it, or `None`.
///
/// Symphonia leaves `bits_per_sample` empty for an ordinary IEEE float WAV although the codec id is
/// F32 or F64, so the depth comes from the codec there. Symphonia also leaves it empty for ALAC,
/// whose magic cookie carries the depth. A lossy codec reports no depth: no container field holds a
/// recording depth for one, and a displayed depth would mislead.
fn bit_depth(params: &AudioCodecParameters) -> Option<i64> {
    if codec_kind(params.codec) == Some(false) {
        return None;
    }
    if let Some(bits) = params.bits_per_sample {
        return Some(i64::from(bits));
    }
    match params.codec {
        CODEC_ID_PCM_F32LE
        | CODEC_ID_PCM_F32BE
        | CODEC_ID_PCM_F32LE_PLANAR
        | CODEC_ID_PCM_F32BE_PLANAR => Some(32),
        CODEC_ID_PCM_F64LE
        | CODEC_ID_PCM_F64BE
        | CODEC_ID_PCM_F64LE_PLANAR
        | CODEC_ID_PCM_F64BE_PLANAR => Some(64),
        CODEC_ID_ALAC => alac_bit_depth(params.extra_data.as_deref()),
        _ => None,
    }
}

/// The ALAC bit depth from its magic cookie.
///
/// The cookie layout is `frame_length` (u32), `compatible_version` (u8), `bit_depth` (u8), `pb`
/// (u8), `mb` (u8), `kb` (u8), `num_channels` (u8), then the remaining fields. The depth is at byte
/// 5, in the range 1 to 32. Symphonia parses the cookie for the channels and the rate but does not
/// copy the depth into `bits_per_sample`.
fn alac_bit_depth(extra_data: Option<&[u8]>) -> Option<i64> {
    let bits = *extra_data?.get(5)?;
    (1..=32).contains(&bits).then(|| i64::from(bits))
}

/// Whether a path has one of the extensions a folder walk searches for.
fn has_audio_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|extension| {
            AUDIO_EXTENSIONS
                .iter()
                .any(|known| known.eq_ignore_ascii_case(extension))
        })
}

/// Collect the files under `roots`, deduplicated and sorted.
fn collect_files(roots: &[PathBuf]) -> Result<Vec<PathBuf>, TriageError> {
    let mut files = Vec::new();
    let mut seen = HashSet::new();
    for root in roots {
        if root.is_file() {
            if seen.insert(root.clone()) {
                files.push(root.clone());
            }
        } else if root.is_dir() {
            walk(root, &mut files, &mut seen)?;
        }
    }
    files.sort();
    Ok(files)
}

/// Recursively collect the audio files under `dir`.
fn walk(
    dir: &Path,
    files: &mut Vec<PathBuf>,
    seen: &mut HashSet<PathBuf>,
) -> Result<(), TriageError> {
    let entries = fs::read_dir(dir).map_err(|source| TriageError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| TriageError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            walk(&path, files, seen)?;
        } else if path.is_file() && has_audio_extension(&path) && seen.insert(path.clone()) {
            files.push(path);
        }
    }
    Ok(())
}

/// The duration of decoded audio in seconds.
fn duration_seconds(audio: &crate::decode::AudioBuffer) -> Option<f64> {
    let rate = audio.sample_rate();
    if rate == 0 {
        return None;
    }
    Some(audio.frames() as f64 / f64::from(rate))
}

/// The average bitrate in kb/s, from the file size and the decoded duration.
///
/// This is the average rate of the whole file. Tags and cover art are part of the file, so they
/// raise it, and a large cover can raise it a lot. It is not the encoded audio rate. It is the only
/// rate available for every codec.
fn bitrate_kbps(size_bytes: i64, duration_seconds: Option<f64>) -> Option<i64> {
    let duration = duration_seconds?;
    if duration <= 0.0 {
        return None;
    }
    Some(((size_bytes as f64 * 8.0) / duration / 1000.0).round() as i64)
}

/// The nanoseconds since the Unix epoch.
///
/// Nanoseconds keep a fast same-size edit visible. Whole seconds would round two nearby edits to
/// the same value and hide the change. Store schema version 2 holds this value.
fn modified_at_nanos(metadata: &fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|elapsed| i64::try_from(elapsed.as_nanos()).ok())
        .unwrap_or_default()
}

/// The whole seconds since the Unix epoch, now.
fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lossy_source_uses_the_high_bitrate_threshold() {
        assert_eq!(verdict_for(Some(false), Some(16_000.0)), Verdict::Restore);
        assert_eq!(
            verdict_for(Some(false), Some(HIGH_BITRATE_CUTOFF_HZ)),
            Verdict::LowBenefit
        );
        assert_eq!(
            verdict_for(Some(false), Some(HIGH_BITRATE_CUTOFF_HZ - 1.0)),
            Verdict::Restore
        );
        assert_eq!(verdict_for(None, Some(16_000.0)), Verdict::Restore);
    }

    #[test]
    fn a_lossless_container_uses_the_suspect_threshold() {
        assert_eq!(
            verdict_for(Some(true), Some(16_000.0)),
            Verdict::SuspectLossless
        );
        assert_eq!(
            verdict_for(Some(true), Some(SUSPECT_LOSSLESS_CUTOFF_HZ)),
            Verdict::Lossless
        );
    }

    #[test]
    fn a_missing_cutoff_is_unreadable() {
        assert_eq!(verdict_for(Some(false), None), Verdict::Unreadable);
    }

    #[test]
    fn the_walk_searches_only_the_listed_extensions() {
        for name in [
            "song.mp3",
            "song.M4A",
            "song.wav",
            "song.aiff",
            "song.aif",
            "song.flac",
            "song.opus",
            "song.ogg",
            "song.webm",
        ] {
            assert!(has_audio_extension(Path::new(name)), "{name}");
        }
        for name in ["song.txt", "song.jpg", "song", "song.mp4"] {
            assert!(!has_audio_extension(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn triage_reports_a_missing_file_with_a_named_error() {
        let error = triage_file(Path::new("does-not-exist.mp3")).expect_err("a missing file");
        assert!(error.to_string().contains("does-not-exist.mp3"));
    }

    #[test]
    fn known_lossless_and_lossy_codecs_are_classified() {
        assert_eq!(is_lossless(CODEC_ID_FLAC, Path::new("x")), Some(true));
        assert_eq!(is_lossless(CODEC_ID_ALAC, Path::new("x")), Some(true));
        assert_eq!(is_lossless(CODEC_ID_PCM_S16LE, Path::new("x")), Some(true));
        assert_eq!(is_lossless(CODEC_ID_MP3, Path::new("x")), Some(false));
        assert_eq!(is_lossless(CODEC_ID_OPUS, Path::new("x")), Some(false));
    }
}
