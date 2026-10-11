//! End to end: load ONNX Runtime from a caller-given library path and restore audio.
//!
//! The library comes from the pinned release that `tools/fetch_onnxruntime.py` fetches
//! into `.cache/onnxruntime/`. The model is the committed task 01 artifact. Both are
//! required; the test fails, and does not skip, when either is missing.

mod common;
mod setup;

use std::env;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use melomae_core::inference::{
    load_onnx_backend, RestoreBackend, RestoreOutcome, RestoreSettings, SAMPLE_RATE,
};

use common::sine;

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

/// Fail with a clear message when the committed model is missing or is an LFS pointer.
fn require_model(path: &Path) {
    let bytes = std::fs::read(path).unwrap_or_else(|error| {
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
}

#[test]
fn e2e_inference_loads_the_runtime_from_a_caller_given_library() {
    let model_path = env::var_os("MELOMAE_APOLLO_ONNX")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root().join("models").join("apollo.onnx"));
    require_model(&model_path);

    let backend = load_onnx_backend(setup::ort_library(), &model_path)
        .expect("load the Apollo model from the fetched runtime");

    let samples = sine(SAMPLE_RATE, 440.0, 0.4, SAMPLE_RATE as usize);
    let outcome = backend
        .restore(
            std::slice::from_ref(&samples),
            &RestoreSettings::default(),
            &mut |_| {},
            &AtomicBool::new(false),
        )
        .expect("restore the one second signal");

    match outcome {
        RestoreOutcome::Complete(channels) => {
            assert_eq!(channels.len(), 1);
            assert_eq!(
                channels[0].len(),
                samples.len(),
                "the output keeps the input length"
            );
            assert!(
                channels[0].iter().all(|sample| sample.is_finite()),
                "the output must be finite"
            );
        }
        RestoreOutcome::Cancelled => panic!("the run must not cancel"),
    }
}
