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

    /// The raw little-endian f32 reference for one file, decoded by ffmpeg's first audio track.
    ///
    /// The decoder selects `-map 0:a:0`, so this is the oracle for the selected track. Pass
    /// `Some("libopus")` to decode Opus with libopus, matching the task's reference decoder.
    pub fn reference_for(
        &self,
        source: &Path,
        rate: u32,
        channels: usize,
        decoder: Option<&str>,
    ) -> Vec<f32> {
        let target = self.dir.join("selected.reference.f32");
        decode_reference(source, &target, rate, channels, decoder);
        read_f32le(&target)
    }

    /// A two-track WebM: track 1 is the default, track 2 is low-delay, as the reviewer built it.
    pub fn two_track_webm(&self, source: &Path) -> PathBuf {
        let target = self.dir.join("two-tracks.webm");
        run(&[
            "-i",
            &source.display().to_string(),
            "-map",
            "0:a:0",
            "-map",
            "0:a:0",
            "-c:a",
            "libopus",
            "-application:a:1",
            "lowdelay",
            &target.display().to_string(),
        ]);
        target
    }

    /// A WebM written with the live muxer option, so its clusters have unknown size.
    pub fn live_webm(&self, source: &Path) -> PathBuf {
        let target = self.dir.join("live.webm");
        run(&[
            "-i",
            &source.display().to_string(),
            "-c:a",
            "libopus",
            "-live",
            "1",
            &target.display().to_string(),
        ]);
        target
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

// --- Builders for the hostile and edge-case WebM inputs of the task 03 fix round ---------------

/// How to rewrite a WebM's `SimpleBlock`s into padded `BlockGroup`s.
#[derive(Clone, Copy)]
pub enum Padding {
    /// Every `SimpleBlock` becomes a `BlockGroup` whose `DiscardPadding` is `i64::MAX`
    /// nanoseconds, the hostile input from `rev03_inputs.py`.
    Hostile,
    /// The first block of the first cluster becomes a `BlockGroup` with this padding, in
    /// nanoseconds; every other block keeps its form.
    Interior(i64),
}

/// Rewrite `source` into `target`, porting the reviewer's `rev03_inputs.py` packing: the EBML
/// header, the Segment's metadata elements, the rewritten clusters, and an empty Cues element.
///
/// ffmpeg writes ordinary audio as `SimpleBlock`s, which carry no padding, so a hostile or
/// interior `DiscardPadding` has to be inserted by rewriting the block into a `BlockGroup`.
pub fn rewrite_webm_padding(source: &Path, target: &Path, padding: Padding) {
    let data = fs::read(source).expect("read the source WebM");
    let top = ebml_elements(&data);
    let header = element_body(&top, 0x1A45_DFA3);
    let segment = element_body(&top, 0x1853_8067);

    let mut metadata: Vec<(u64, Vec<u8>)> = Vec::new();
    let mut clusters: Vec<Vec<u8>> = Vec::new();
    for (id, body) in ebml_elements(&segment) {
        if id == 0x1F43_B675 {
            clusters.push(rewrite_cluster(&body, padding, clusters.is_empty()));
        } else if id != 0x114D_9B74 && id != 0x1C53_BB6B {
            metadata.push((id, body));
        }
    }

    let mut body = Vec::new();
    for (id, element) in &metadata {
        body.extend_from_slice(&encode_element(*id, element));
    }
    for cluster in &clusters {
        body.extend_from_slice(&encode_element(0x1F43_B675, cluster));
    }
    body.extend_from_slice(&encode_element(0x1C53_BB6B, &[]));

    let mut out = encode_element(0x1A45_DFA3, &header);
    out.extend_from_slice(&encode_element(0x1853_8067, &body));
    fs::write(target, out).expect("write the rewritten WebM");
}

/// Rewrite the `SimpleBlock`s of one cluster. `Padding::Hostile` rewrites every block;
/// `Padding::Interior` rewrites only the first block of the first cluster.
fn rewrite_cluster(cluster: &[u8], padding: Padding, first_cluster: bool) -> Vec<u8> {
    let mut out = Vec::new();
    let mut applied = false;
    for (id, body) in ebml_elements(cluster) {
        let rewrite = id == 0xA3
            && match padding {
                Padding::Hostile => true,
                Padding::Interior(_) => first_cluster && !applied,
            };
        if rewrite {
            let value = match padding {
                Padding::Hostile => i64::MAX,
                Padding::Interior(nanoseconds) => nanoseconds,
            };
            let mut group = encode_element(0xA1, &body);
            group.extend_from_slice(&encode_element(0x75A2, &value.to_be_bytes()));
            out.extend_from_slice(&encode_element(0xA0, &group));
            applied = true;
        } else {
            out.extend_from_slice(&encode_element(id, &body));
        }
    }
    out
}

/// Parse an EBML document into its (element id, body) pairs.
fn ebml_elements(data: &[u8]) -> Vec<(u64, Vec<u8>)> {
    let mut elements = Vec::new();
    let mut pos = 0usize;
    while pos < data.len() {
        let Some(id) = read_vint(data, &mut pos) else {
            break;
        };
        let Some((size, unknown)) = read_vint_size(data, &mut pos) else {
            break;
        };
        // An unknown size runs to the end of the containing data.
        let size = if unknown || size == 0 {
            data.len().saturating_sub(pos)
        } else {
            match usize::try_from(size) {
                Ok(size) => size,
                Err(_) => break,
            }
        };
        let Some(end) = pos.checked_add(size) else {
            break;
        };
        let Some(body) = data.get(pos..end) else {
            break;
        };
        elements.push((id, body.to_vec()));
        pos = end;
    }
    elements
}

/// The body of the first element with `id`.
fn element_body(elements: &[(u64, Vec<u8>)], id: u64) -> Vec<u8> {
    elements
        .iter()
        .find(|(element_id, _)| *element_id == id)
        .expect("the element is present")
        .1
        .clone()
}

/// Read an EBML element id, marker included.
fn read_vint(data: &[u8], pos: &mut usize) -> Option<u64> {
    let first = *data.get(*pos)?;
    if first == 0 {
        return None;
    }
    let len = first.leading_zeros() as usize + 1;
    let mut value = 0u64;
    for offset in 0..len {
        value = (value << 8) | u64::from(*data.get(*pos + offset)?);
    }
    *pos += len;
    Some(value)
}

/// Read an EBML element size, with the marker bits removed. The flag reports the unknown-size
/// marker (every value bit set), which means "to the end of the containing data".
fn read_vint_size(data: &[u8], pos: &mut usize) -> Option<(u64, bool)> {
    let first = *data.get(*pos)?;
    if first == 0 {
        return None;
    }
    let len = first.leading_zeros() as usize + 1;
    if len > 8 {
        return None;
    }
    let marker = (1u16 << (8 - len)) - 1;
    let mut value = u64::from(first) & u64::from(marker as u8);
    let mut all_ones = value == u64::from(marker as u8);
    for offset in 1..len {
        let byte = *data.get(*pos + offset)?;
        value = (value << 8) | u64::from(byte);
        if byte != 0xFF {
            all_ones = false;
        }
    }
    *pos += len;
    Some((value, all_ones))
}

/// Encode an EBML element id.
fn encode_id(id: u64) -> Vec<u8> {
    let bytes = id.to_be_bytes();
    let first = bytes.iter().position(|byte| *byte != 0).unwrap_or(7);
    bytes[first..].to_vec()
}

/// Encode an EBML element size with the shortest vint that fits.
fn encode_size(len: usize) -> Vec<u8> {
    for width in 1..=8u32 {
        let max = (1u128 << (7 * width)) - 1;
        if (len as u128) < max {
            let value = (1u128 << (7 * width)) | len as u128;
            return value.to_be_bytes()[(16 - width as usize)..].to_vec();
        }
    }
    panic!("the element is too large to encode");
}

/// Encode an EBML element with a known size.
fn encode_element(id: u64, body: &[u8]) -> Vec<u8> {
    let mut out = encode_id(id);
    out.extend_from_slice(&encode_size(body.len()));
    out.extend_from_slice(body);
    out
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
