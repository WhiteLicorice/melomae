//! Fix round 1 for task 16: window selection over the whole file, every channel at each position,
//! a sub-second modification time, and the bit depth of float PCM.
//!
//! Every input is built in test setup and removed in teardown. Nothing is committed.

mod common;
mod setup;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use melomae_core::decode::decode_file;
use melomae_core::store::{Store, Verdict};
use melomae_core::triage::{scan_roots, triage_file};

use common::write_wav_f32;
use setup::{calibration_source_path, TempDir};

/// A temporary database file that is deleted when it drops.
struct TempDb {
    path: PathBuf,
}

impl TempDb {
    fn new(tag: &str) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};

        static COUNTER: AtomicU64 = AtomicU64::new(0);

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut path = std::env::temp_dir();
        path.push(format!(
            "melomae-fix16-{tag}-{}-{nanos}-{sequence}.db",
            std::process::id()
        ));
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        for suffix in ["-journal", "-wal", "-shm"] {
            let mut sidecar = self.path.clone().into_os_string();
            sidecar.push(suffix);
            let _ = fs::remove_file(PathBuf::from(sidecar));
        }
    }
}

/// The committed calibration source, decoded to planar f32 at 44.1 kHz.
fn source() -> Vec<Vec<f32>> {
    decode_file(calibration_source_path())
        .expect("decode the calibration source")
        .channels()
        .to_vec()
}

/// Set a file's modification time.
fn set_modified(path: &Path, time: SystemTime) {
    fs::File::options()
        .write(true)
        .open(path)
        .expect("open a file to set its time")
        .set_modified(time)
        .expect("set the modification time");
}

/// Encode `source` to `target` with the pinned ffmpeg.
fn encode(source: &Path, target: &Path, arguments: &[&str]) {
    let status = Command::new(setup::ffmpeg())
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(source)
        .args(arguments)
        .arg(target)
        .status()
        .expect("run ffmpeg");
    assert!(status.success(), "ffmpeg failed for {}", target.display());
}

/// A running-average low-pass, applied `passes` times. Each pass has `taps` taps.
fn low_pass(samples: &[f32], taps: usize, passes: usize) -> Vec<f32> {
    let mut output = samples.to_vec();
    for _ in 0..passes {
        let mut accumulator = 0.0f32;
        let mut next = vec![0.0f32; output.len()];
        for (index, value) in output.iter().enumerate() {
            accumulator += *value;
            if index >= taps {
                accumulator -= output[index - taps];
            }
            next[index] = accumulator / taps.min(index + 1) as f32;
        }
        output = next;
    }
    output
}

/// The source with an opening prepended: `26` seconds of `opening`, then the source.
fn with_opening(opening: impl Fn(&[f32]) -> Vec<f32>) -> Vec<Vec<f32>> {
    let opening_frames = 44_100 * 26;
    source()
        .iter()
        .map(|plane| {
            let head: Vec<f32> = opening(plane)
                .into_iter()
                .cycle()
                .take(opening_frames)
                .collect();
            let mut out = head;
            out.extend_from_slice(plane);
            out
        })
        .collect()
}

#[test]
fn triage_window_selection_finds_audio_after_a_long_silent_opening() {
    let dir = TempDir::new("melomae-fix16-intro");
    let path = dir.join("music.wav");
    write_wav_f32(
        &path,
        44_100,
        &with_opening(|plane| vec![0.0f32; plane.len()]),
    );

    let item = triage_file(&path).expect("triage the file");

    assert_eq!(item.verdict, Verdict::Lossless, "{item:?}");
    assert!(
        item.spectral_cutoff_hz.unwrap_or(0.0) >= 21_000.0,
        "the full-band audio after the silence must set the cutoff, got {:?}",
        item.spectral_cutoff_hz
    );
}

#[test]
fn triage_window_selection_finds_full_band_audio_after_a_narrow_band_opening() {
    let dir = TempDir::new("melomae-fix16-narrow");
    let path = dir.join("music.wav");
    write_wav_f32(&path, 44_100, &with_opening(|plane| low_pass(plane, 32, 3)));

    let item = triage_file(&path).expect("triage the file");

    assert_eq!(item.verdict, Verdict::Lossless, "{item:?}");
    assert!(
        item.spectral_cutoff_hz.unwrap_or(0.0) >= 21_000.0,
        "the full-band audio after the narrow-band opening must set the cutoff, got {:?}",
        item.spectral_cutoff_hz
    );
}

#[test]
fn triage_window_selection_reads_the_active_channel() {
    let dir = TempDir::new("melomae-fix16-channels");
    let active: Vec<f32> = source()[0]
        .iter()
        .copied()
        .cycle()
        .take(44_100 * 30)
        .collect();
    let silent = vec![0.0f32; active.len()];

    let right_path = dir.join("right.wav");
    write_wav_f32(&right_path, 44_100, &[silent.clone(), active.clone()]);
    let item = triage_file(&right_path).expect("triage the silent-left file");
    assert_eq!(item.verdict, Verdict::Lossless, "silent left: {item:?}");
    assert!(
        item.spectral_cutoff_hz.unwrap_or(0.0) >= 21_000.0,
        "silent left: {:?}",
        item.spectral_cutoff_hz
    );

    let left_path = dir.join("left.wav");
    write_wav_f32(&left_path, 44_100, &[active, silent]);
    let item = triage_file(&left_path).expect("triage the silent-right file");
    assert_eq!(item.verdict, Verdict::Lossless, "silent right: {item:?}");
    assert!(
        item.spectral_cutoff_hz.unwrap_or(0.0) >= 21_000.0,
        "silent right: {:?}",
        item.spectral_cutoff_hz
    );
}

#[test]
fn triage_rescan_sees_time_and_size_changes() {
    let dir = TempDir::new("melomae-fix16-rescan");
    let path = dir.join("song.wav");
    let planes = source();
    write_wav_f32(&path, 44_100, &planes);

    let second = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let base = UNIX_EPOCH + Duration::from_secs(second);
    set_modified(&path, base + Duration::from_millis(100));

    let db = TempDb::new("rescan");
    let store = Store::open(db.path()).unwrap();
    let roots = [path.clone()];
    assert_eq!(scan_roots(&store, &roots).unwrap().scanned, 1);

    // An unchanged file is still skipped.
    let unchanged = scan_roots(&store, &roots).unwrap();
    assert_eq!(unchanged.scanned, 0);
    assert_eq!(unchanged.skipped, 1);

    // A same-size edit with a sub-second modification-time change is rescanned.
    let size = fs::metadata(&path).unwrap().len();
    let silent = vec![vec![0.0f32; planes[0].len()]; planes.len()];
    write_wav_f32(&path, 44_100, &silent);
    set_modified(&path, base + Duration::from_millis(800));
    assert_eq!(size, fs::metadata(&path).unwrap().len());
    assert_eq!(
        scan_roots(&store, &roots).unwrap().scanned,
        1,
        "a same-size sub-second edit must be rescanned"
    );

    // A time-only change with a sub-second step is rescanned.
    set_modified(&path, base + Duration::from_millis(900));
    assert_eq!(
        scan_roots(&store, &roots).unwrap().scanned,
        1,
        "a sub-second time-only change must be rescanned"
    );

    // A size-only change is rescanned.
    let shorter = vec![vec![0.0f32; planes[0].len() / 2]; planes.len()];
    write_wav_f32(&path, 44_100, &shorter);
    set_modified(&path, base + Duration::from_millis(900));
    let size_only = scan_roots(&store, &roots).unwrap();
    assert_eq!(size_only.scanned, 1, "a size-only change must be rescanned");
}

#[test]
fn triage_reports_float_pcm_bit_depth() {
    let dir = TempDir::new("melomae-fix16-depth");
    let source_wav = dir.join("source.wav");
    write_wav_f32(&source_wav, 44_100, &source());

    assert_eq!(
        triage_file(&source_wav).unwrap().bit_depth,
        Some(32),
        "a 32-bit float WAV has a known depth"
    );

    let f64_wav = dir.join("f64.wav");
    encode(&source_wav, &f64_wav, &["-c:a", "pcm_f64le"]);
    assert_eq!(
        triage_file(&f64_wav).unwrap().bit_depth,
        Some(64),
        "a 64-bit float WAV has a known depth"
    );
}

#[test]
fn triage_keeps_integer_and_lossless_bit_depth_and_hides_lossy_depth() {
    let dir = TempDir::new("melomae-fix16-depth2");
    let source_wav = dir.join("source.wav");
    write_wav_f32(&source_wav, 44_100, &source());

    let s16 = dir.join("s16.wav");
    encode(&source_wav, &s16, &["-c:a", "pcm_s16le"]);
    assert_eq!(triage_file(&s16).unwrap().bit_depth, Some(16));

    let s24 = dir.join("s24.wav");
    encode(&source_wav, &s24, &["-c:a", "pcm_s24le"]);
    assert_eq!(triage_file(&s24).unwrap().bit_depth, Some(24));

    let flac = dir.join("a.flac");
    encode(&source_wav, &flac, &["-c:a", "flac", "-sample_fmt", "s16"]);
    assert_eq!(triage_file(&flac).unwrap().bit_depth, Some(16));

    let alac = dir.join("a.m4a");
    encode(&source_wav, &alac, &["-c:a", "alac", "-sample_fmt", "s16p"]);
    assert_eq!(
        triage_file(&alac).unwrap().bit_depth,
        Some(16),
        "ALAC's depth comes from its magic cookie"
    );

    let mp3 = dir.join("a.mp3");
    encode(&source_wav, &mp3, &["-c:a", "libmp3lame", "-b:a", "128k"]);
    assert_eq!(
        triage_file(&mp3).unwrap().bit_depth,
        None,
        "a lossy codec has no recording depth"
    );
}
