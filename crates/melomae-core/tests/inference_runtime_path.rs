//! After a successful load, a different or missing runtime path is rejected.
//!
//! The runtime is process-wide, so this test lives in its own binary and loads the pinned
//! library once. A later call that names a different library, or a path that does not exist,
//! must return a typed error that names the loaded library and the requested path. Before the
//! fix the second call returned `Ok` and silently ignored the requested path.

mod common;
mod setup;

use melomae_core::inference::{init_runtime, InferenceError};

use setup::TempDir;

#[test]
fn a_different_inference_runtime_path_after_a_successful_load_is_rejected() {
    let loaded = std::fs::canonicalize(setup::ort_library()).expect("resolve the pinned library");
    init_runtime(setup::ort_library()).expect("the pinned library must load");

    let dir = TempDir::new("melomae-ort-different");
    let other = dir.join("libonnxruntime.so");
    std::fs::write(&other, b"not an onnx runtime library").expect("write the fake library");

    let error = init_runtime(&other).expect_err("a different library must be rejected");
    assert!(
        matches!(error, InferenceError::Runtime(_)),
        "a different library must return a typed Runtime error, got: {error}"
    );
    let message = error.to_string();
    assert!(
        message.contains(&loaded.display().to_string()),
        "the error must name the loaded library, got: {message}"
    );
    assert!(
        message.contains(&other.display().to_string()),
        "the error must name the requested library, got: {message}"
    );

    let missing = dir.join("missing-onnxruntime.so");
    let error = init_runtime(&missing).expect_err("a missing library must be rejected");
    assert!(
        matches!(error, InferenceError::Runtime(_)),
        "a missing library must return a typed Runtime error, got: {error}"
    );
    let message = error.to_string();
    assert!(
        message.contains(&loaded.display().to_string()),
        "the error must name the loaded library, got: {message}"
    );
    assert!(
        message.contains(&missing.display().to_string()),
        "the error must name the requested library, got: {message}"
    );
}
