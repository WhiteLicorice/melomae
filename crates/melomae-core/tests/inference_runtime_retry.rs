//! A repeated failed ONNX Runtime load reports a typed error and does not abort.
//!
//! The runtime is process-wide, so this test lives in its own binary. It makes two failed
//! loads in one process through the public entry. Before the fix the second load re-entered
//! ONNX Runtime's once-cell over uninitialized data and the process aborted.

mod common;
mod setup;

use melomae_core::inference::{load_onnx_backend, InferenceError};

use setup::TempDir;

#[test]
fn a_repeated_failed_inference_runtime_load_reports_a_typed_error_and_does_not_abort() {
    let dir = TempDir::new("melomae-ort-retry");
    let library = dir.join("libonnxruntime.so");
    std::fs::write(&library, b"not an onnx runtime library").expect("write the fake library");
    let model = dir.join("apollo.onnx");

    for attempt in 1..=2 {
        let error = match load_onnx_backend(&library, &model) {
            Ok(_) => panic!("attempt {attempt}: a wrong library must fail"),
            Err(error) => error,
        };
        assert!(
            matches!(error, InferenceError::Runtime(_)),
            "attempt {attempt}: a wrong library must return a typed Runtime error, got: {error}"
        );
    }
}
