//! Build the decode test inputs in a temporary directory, and remove them when the test ends.
//!
//! The inputs are synthesized in Rust and encoded with the pinned ffmpeg, so nothing is committed
//! and nothing is read from outside the repository. ffmpeg is development and CI only: the
//! decoder never runs it. `python tools/fetch_ffmpeg.py` installs the pinned build; when it is
//! missing, the tests that need it fail with that command. A green run therefore means every
//! format was decoded.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use crate::common::{log_sweep, read_f32le, sine, write_wav_f32};

/// The number of seconds of audio in every synthesized source.
const SOURCE_SECONDS: f64 = 6.0;

/// The swr settings the owner's old pipeline used (`C:\Lab\Apollo\inference2.py:39`).
const SWR_FILTER: &str = "aresample=resampler=swr:filter_size=256:phase_shift=24:cutoff=0.98";

/// The inputs to encode, the source rate each is encoded from, and its channel count.
pub const INPUTS: &[(&str, u32, usize)] = &[
    ("wav_44100.wav", 44_100, 2),
    ("aiff_44100.aiff", 44_100, 2),
    ("flac_44100.flac", 44_100, 2),
    ("mp3_44100.mp3", 44_100, 2),
    ("aac_44100.m4a", 44_100, 2),
    ("alac_44100.m4a", 44_100, 2),
    ("vorbis_44100.ogg", 44_100, 2),
    ("opus_48000.opus", 48_000, 2),
    ("opus_48000.webm", 48_000, 2),
];

/// Encoded inputs that re-use an Opus or FLAC encode under an awkward file name.
pub const AWKWARD_NAMES: &[(&str, &str)] = &[
    ("Florida!!!.opus", "opus_48000.opus"),
    ("Habits (Stay High).opus", "opus_48000.opus"),
    ("Sigur Rós.flac", "flac_44100.flac"),
];

/// A temporary directory that is removed when it is dropped, including during an unwind.
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// Create a fresh unique temporary directory.
    pub fn new(prefix: &str) -> TempDir {
        use std::time::{SystemTime, UNIX_EPOCH};
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("{prefix}-{}-{stamp}", std::process::id()));
        fs::create_dir_all(&path).expect("create the temporary directory");
        TempDir { path }
    }

    /// The directory path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The path to a file inside the directory.
    pub fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// The standard fixture set, in a temporary directory that is removed when the guard drops.
pub struct Fixtures {
    dir: TempDir,
}

impl Fixtures {
    /// Build the standard fixture set: every encoded input and its reference decode.
    pub fn build() -> Fixtures {
        let fixtures = Fixtures {
            dir: TempDir::new("melomae-decode"),
        };
        fixtures.encode_all();
        fixtures
    }

    /// The fixture directory.
    pub fn dir(&self) -> &Path {
        self.dir.path()
    }

    /// The absolute path to a fixture.
    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// The raw little-endian f32 reference decode for an input.
    pub fn reference(&self, name: &str) -> Vec<f32> {
        read_f32le(&self.path(&format!("{name}.ref.f32")))
    }

    /// Synthesize a stereo log sweep, write it, and return its path.
    pub fn sweep_wav(&self, rate: u32, seconds: f64, start: f64, end: f64) -> PathBuf {
        let path = self.dir.join(&format!("sweep_{rate}.wav"));
        let frames = (f64::from(rate) * seconds).round() as usize;
        let sweep = log_sweep(rate, start, end, 0.5, frames);
        write_wav_f32(&path, rate, &[sweep.clone(), sweep]);
        path
    }

    /// Resample `source` with the ffmpeg swr settings, as a raw f32 reference.
    pub fn swr_reference(&self, source: &Path, rate: u32, channels: usize) -> Vec<f32> {
        let target = self.dir.join("swr.reference.f32");
        run(&[
            "-i",
            &source.display().to_string(),
            "-af",
            SWR_FILTER,
            "-ar",
            &rate.to_string(),
            "-ac",
            &channels.to_string(),
            "-f",
            "f32le",
            "-c:a",
            "pcm_f32le",
            &target.display().to_string(),
        ]);
        read_f32le(&target)
    }

    fn encode_all(&self) {
        for (name, rate, channels) in INPUTS {
            let source = self.source_wav(*rate);
            encode(&source, &self.path(name), encoder_args(name));
            self.write_reference(name, *rate, *channels);
        }
        for (name, from) in AWKWARD_NAMES {
            fs::copy(self.path(from), self.path(name)).expect("copy the awkward-named input");
            let rate = if name.ends_with(".opus") {
                48_000
            } else {
                44_100
            };
            self.write_reference(name, rate, 2);
        }
    }

    /// Synthesize (once per rate) and write the stereo source WAV.
    fn source_wav(&self, rate: u32) -> PathBuf {
        let path = self.dir.join(&format!("source_{rate}.wav"));
        if !path.is_file() {
            let planes = source(rate, SOURCE_SECONDS);
            write_wav_f32(&path, rate, &planes);
        }
        path
    }

    fn write_reference(&self, name: &str, rate: u32, channels: usize) {
        let decoder = if name.ends_with(".opus") {
            Some("libopus")
        } else {
            None
        };
        decode_reference(
            &self.path(name),
            &self.path(&format!("{name}.ref.f32")),
            rate,
            channels,
            decoder,
        );
    }
}

/// The encoder arguments for an input, keyed by its file name.
fn encoder_args(name: &str) -> Vec<&'static str> {
    match name {
        "wav_44100.wav" => vec!["-c:a", "pcm_f32le"],
        "aiff_44100.aiff" => vec!["-c:a", "pcm_s24be"],
        "flac_44100.flac" => vec!["-c:a", "flac"],
        "mp3_44100.mp3" => vec!["-c:a", "libmp3lame", "-b:a", "320k"],
        "aac_44100.m4a" => vec!["-c:a", "aac", "-b:a", "256k"],
        "alac_44100.m4a" => vec!["-c:a", "alac"],
        "vorbis_44100.ogg" => vec!["-c:a", "libvorbis", "-q:a", "6"],
        "opus_48000.opus" | "opus_48000.webm" => vec!["-c:a", "libopus", "-b:a", "256k"],
        other => panic!("no encoder is registered for {other}"),
    }
}

/// The pinned BtbN monthly-archive tag. Keep this in step with `tools/fetch_ffmpeg.py`.
pub const FFMPEG_TAG: &str = "autobuild-2026-08-31-13-27";

/// The build id inside that archive, checked against `ffmpeg -version`.
pub const FFMPEG_BUILD: &str = "N-126342-gf88b741dbf";

/// The repository root, from the core crate's manifest directory.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate is two levels below the repository root")
        .to_path_buf()
}

/// The ffmpeg the tests use: the pinned build fetched into `.cache/ffmpeg/<tag>/`.
///
/// The tests never read `PATH` or `MELOMAE_FFMPEG`. When the pinned build is missing they fail
/// with the command that fetches it.
pub fn ffmpeg() -> &'static Path {
    static FFMPEG: OnceLock<PathBuf> = OnceLock::new();
    FFMPEG.get_or_init(|| {
        let tag_dir = repo_root().join(".cache").join("ffmpeg").join(FFMPEG_TAG);
        let binary = find_binary(&tag_dir).unwrap_or_else(|| {
            panic!(
                "the pinned ffmpeg is not installed at {}.\nRun: python tools/fetch_ffmpeg.py",
                tag_dir.display()
            )
        });

        let output = Command::new(&binary)
            .arg("-version")
            .output()
            .unwrap_or_else(|e| panic!("run {}: {e}", binary.display()));
        assert!(output.status.success(), "{} -version failed with {}", binary.display(), output.status);

        let version = String::from_utf8_lossy(&output.stdout);
        let first_line = version.lines().next().unwrap_or_default();
        assert!(
            first_line.contains(FFMPEG_BUILD),
            "the fetched ffmpeg is not the pinned build.\n  found:    {first_line}\n  expected: {FFMPEG_BUILD}\n\
             Delete .cache/ffmpeg and run: python tools/fetch_ffmpeg.py"
        );

        binary
    })
}

/// The ffmpeg binary inside an extracted tag directory, if the fetch has run.
fn find_binary(tag_dir: &Path) -> Option<PathBuf> {
    for entry in fs::read_dir(tag_dir).ok()?.flatten() {
        for name in ["ffmpeg.exe", "ffmpeg"] {
            let candidate = entry.path().join("bin").join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Run ffmpeg, failing the test when it exits non-zero.
fn run(args: &[&str]) {
    let output = Command::new(ffmpeg())
        .args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(args)
        .output()
        .expect("run ffmpeg");
    assert!(
        output.status.success(),
        "ffmpeg {args:?} failed with {}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Encode `source` into `target` with the given encoder arguments.
fn encode(source: &Path, target: &Path, args: Vec<&'static str>) {
    let mut full = vec!["-i".to_string(), source.display().to_string()];
    full.extend(args.iter().map(|a| a.to_string()));
    full.push(target.display().to_string());
    let borrowed: Vec<&str> = full.iter().map(String::as_str).collect();
    run(&borrowed);
}

/// Decode `source` into a raw little-endian f32 reference at the given rate and channel count.
fn decode_reference(
    source: &Path,
    target: &Path,
    rate: u32,
    channels: usize,
    decoder: Option<&str>,
) {
    let mut full: Vec<String> = Vec::new();
    if let Some(decoder) = decoder {
        full.extend(["-c:a".to_string(), decoder.to_string()]);
    }
    full.extend([
        "-i".to_string(),
        source.display().to_string(),
        "-map".to_string(),
        "0:a:0".to_string(),
        "-ar".to_string(),
        rate.to_string(),
        "-ac".to_string(),
        channels.to_string(),
        "-f".to_string(),
        "f32le".to_string(),
        "-c:a".to_string(),
        "pcm_f32le".to_string(),
        target.display().to_string(),
    ]);
    let borrowed: Vec<&str> = full.iter().map(String::as_str).collect();
    run(&borrowed);
}

/// Low-level noise from a fixed seed, so the source is deterministic. Amplitude is +/-0.01.
fn noise(frames: usize, seed: u64) -> Vec<f32> {
    let mut state = seed;
    (0..frames)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (((state >> 40) as f32 / (1u32 << 24) as f32) - 0.5) * 0.02
        })
        .collect()
}

/// A deterministic stereo source: a tone and a log sweep per channel, plus low-level noise.
fn source(rate: u32, seconds: f64) -> Vec<Vec<f32>> {
    let frames = (f64::from(rate) * seconds).round() as usize;
    let left = sine(rate, 440.0, 0.4, frames);
    let right = sine(rate, 554.37, 0.4, frames);
    let sweep = log_sweep(rate, 50.0, 15_000.0, 0.3, frames);
    let left_noise = noise(frames, 0x1234_5678);
    let right_noise = noise(frames, 0x9abc_def0);
    let mix = |tone: &[f32], noise: &[f32]| -> Vec<f32> {
        (0..frames).map(|i| tone[i] + sweep[i] + noise[i]).collect()
    };
    vec![mix(&left, &left_noise), mix(&right, &right_noise)]
}
