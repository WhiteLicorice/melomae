//! Parity of the Rust ONNX engine against the stock Apollo golden.
//!
//! The model and the parity input are committed through Git LFS. The test
//! reads the model from the path in `MELOMAE_APOLLO_ONNX`, or from
//! `models/apollo.onnx` under the repository root. It reads the input from
//! `tools/parity/input_wav.wav`. It fails, and does not skip, when any of the
//! three committed files is missing or is a Git LFS pointer, with a message
//! that says to run `git lfs pull`.
//!
//! The 18 s input is three copies of the input WAV. The test builds it in a
//! temporary directory and removes it when the test ends.
//! `tools/parity/make_golden.py` builds the same input in the same way: both
//! read the committed input to 44.1 kHz float frames and concatenate the
//! frames three times. The input is a 32-bit float stereo WAV, so both sides
//! read the identical sample values.

mod common;
mod setup;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use melomae_core::decode::decode_file;
use melomae_core::inference::{
    load_onnx_backend, RestoreBackend, RestoreOutcome, RestoreSettings, SAMPLE_RATE,
};

use common::{max_abs_diff, planar_from_interleaved, write_wav_f32};
use setup::TempDir;

/// The task 01 artifact SHA-256, from `docs/evidence/01-onnx-spike.md`.
const MODEL_SHA256: &str = "fdf62f6c0ba5daf3a38c1761443456e4656707218b91a19cdad41a5d4bad78b9";

/// The SRS §12.1 parity limit on the maximum absolute difference.
const MAX_ABS_LIMIT: f32 = 1e-3;

/// The SRS §12.1 parity limit on the root-mean-square error.
const RMSE_LIMIT: f64 = 1e-4;

/// The chunk settings of the golden run (task 04 scope 10).
const CHUNK_SECONDS: f64 = 6.0;
const OVERLAP_SECONDS: f64 = 1.0;
const CHUNK_PAD_SECONDS: f64 = 1.0;

/// The Git LFS pointer header. A checked-out pointer starts with this line.
const LFS_POINTER: &[u8] = b"version https://git-lfs.github.com/spec/v1";

/// The repository root, from the core crate's manifest directory.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate is two levels below the repository root")
        .to_path_buf()
}

/// Read a committed file, and stop with `git lfs pull` guidance when it is
/// missing or is a Git LFS pointer.
fn committed_bytes(path: &Path) -> Vec<u8> {
    let bytes = fs::read(path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}). Run: git lfs pull",
            path.display()
        )
    });
    assert!(
        !bytes.starts_with(LFS_POINTER),
        "{} is a Git LFS pointer, not the data. Run: git lfs pull",
        path.display()
    );
    bytes
}

/// Check a committed file against its SHA-256 sidecar.
fn check_sidecar(path: &Path, bytes: &[u8]) {
    let sidecar = PathBuf::from(format!("{}.sha256", path.display()));
    let expected = fs::read_to_string(&sidecar)
        .unwrap_or_else(|error| panic!("read {} ({error})", sidecar.display()))
        .trim()
        .to_ascii_lowercase();
    assert_eq!(
        sha256_bytes(bytes),
        expected,
        "the bytes of {} do not match {}. The file is not the committed data.",
        path.display(),
        sidecar.display()
    );
}

/// The SHA-256 of some bytes.
fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The rate, channel count, and interleaved samples of a 32-bit float WAV.
fn read_float_wav(bytes: &[u8]) -> (u32, usize, Vec<f32>) {
    assert!(
        bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE",
        "the golden is not a RIFF/WAVE file"
    );
    let mut pos = 12usize;
    let mut format: Option<(u16, u16, u32, u16)> = None;
    let mut data: Option<&[u8]> = None;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let start = pos + 8;
        let end = start + size;
        if end > bytes.len() {
            break;
        }
        match id {
            b"fmt " => {
                let tag = u16::from_le_bytes(bytes[start..start + 2].try_into().unwrap());
                let channels = u16::from_le_bytes(bytes[start + 2..start + 4].try_into().unwrap());
                let rate = u32::from_le_bytes(bytes[start + 4..start + 8].try_into().unwrap());
                let bits = u16::from_le_bytes(bytes[start + 14..start + 16].try_into().unwrap());
                format = Some((tag, channels, rate, bits));
            }
            b"data" => data = Some(&bytes[start..end]),
            _ => {}
        }
        pos = end + (size & 1);
    }
    let (tag, channels, rate, bits) = format.expect("the golden has a fmt chunk");
    assert_eq!(tag, 3, "the golden must be IEEE float");
    assert_eq!(bits, 32, "the golden must be 32-bit float");
    let data = data.expect("the golden has a data chunk");
    let samples = data
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    (rate, channels as usize, samples)
}

/// The root-mean-square error between two equally long buffers.
fn rmse(a: &[f32], b: &[f32]) -> f64 {
    assert_eq!(a.len(), b.len(), "buffers must be the same length");
    let sum: f64 = a
        .iter()
        .zip(b)
        .map(|(x, y)| {
            let diff = f64::from(*x) - f64::from(*y);
            diff * diff
        })
        .sum();
    (sum / a.len() as f64).sqrt()
}

/// Three copies of the source WAV's frames, per channel.
fn tiled_18s(source: &[Vec<f32>]) -> Vec<Vec<f32>> {
    source
        .iter()
        .map(|channel| {
            let mut tiled = Vec::with_capacity(channel.len() * 3);
            for _ in 0..3 {
                tiled.extend_from_slice(channel);
            }
            tiled
        })
        .collect()
}

#[test]
fn inference_parity_with_stock_apollo_on_the_18s_input() {
    let input_path = repo_root()
        .join("tools")
        .join("parity")
        .join("input_wav.wav");
    let golden_path = repo_root().join("tools").join("parity").join("golden.wav");

    let input_bytes = committed_bytes(&input_path);
    check_sidecar(&input_path, &input_bytes);
    let golden_bytes = committed_bytes(&golden_path);
    check_sidecar(&golden_path, &golden_bytes);

    let model_path = env::var_os("MELOMAE_APOLLO_ONNX")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root().join("models").join("apollo.onnx"));
    let model_bytes = committed_bytes(&model_path);
    check_sidecar(&model_path, &model_bytes);
    assert_eq!(
        sha256_bytes(&model_bytes),
        MODEL_SHA256,
        "the model does not match the task 01 artifact"
    );

    let (rate, channels, interleaved) = read_float_wav(&golden_bytes);
    assert_eq!(rate, SAMPLE_RATE);
    assert_eq!(channels, 2, "the golden is stereo");
    let golden = planar_from_interleaved(&interleaved, channels);

    let dir = TempDir::new("melomae-inference-parity");
    let source = decode_file(&input_path).expect("decode the committed input WAV");
    assert_eq!(source.sample_rate(), SAMPLE_RATE);
    let tiled = tiled_18s(source.channels());
    let tiled_path = dir.join("input_18s.wav");
    write_wav_f32(&tiled_path, SAMPLE_RATE, &tiled);
    let audio = decode_file(&tiled_path).expect("decode the 18 s input");
    assert_eq!(
        audio.channels(),
        &tiled[..],
        "the 18 s input must round-trip through the WAV writer"
    );
    let input = audio.channels().to_vec();

    let backend =
        load_onnx_backend(setup::ort_library(), &model_path).expect("load the Apollo ONNX model");
    let settings = RestoreSettings {
        chunk_seconds: Some(CHUNK_SECONDS),
        overlap_seconds: OVERLAP_SECONDS,
        chunk_pad_seconds: CHUNK_PAD_SECONDS,
    };
    let outcome = backend
        .restore(
            &input,
            &settings,
            &mut |_| {},
            &std::sync::atomic::AtomicBool::new(false),
        )
        .expect("run the restore");
    let restored = match outcome {
        RestoreOutcome::Complete(channels) => channels,
        RestoreOutcome::Cancelled => panic!("the run must not cancel"),
    };

    assert_eq!(restored.len(), golden.len(), "channel count");
    for (channel, (got, want)) in restored.iter().zip(&golden).enumerate() {
        assert_eq!(got.len(), want.len(), "channel {channel} length");
        let max_abs = max_abs_diff(got, want);
        let channel_rmse = rmse(got, want);
        println!(
            "channel {channel}: max abs {max_abs:.6e}, RMSE {channel_rmse:.6e}, \
             limits {MAX_ABS_LIMIT:.0e} and {RMSE_LIMIT:.0e}"
        );
        assert!(
            max_abs <= MAX_ABS_LIMIT,
            "channel {channel} max abs {max_abs:e} exceeds {MAX_ABS_LIMIT:e}"
        );
        assert!(
            channel_rmse <= RMSE_LIMIT,
            "channel {channel} RMSE {channel_rmse:e} exceeds {RMSE_LIMIT:e}"
        );
    }
}
