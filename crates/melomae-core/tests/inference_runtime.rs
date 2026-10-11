//! The dynamic ONNX Runtime load reports a typed error, and never panics.
//!
//! The engine loads ONNX Runtime at run time from a caller-given library path (task 33).
//! These tests live in their own binary, because the runtime initializes once per process.

mod common;
mod setup;

use melomae_core::inference::{init_runtime, load_onnx_backend, InferenceError};

use setup::TempDir;

#[test]
fn a_missing_inference_runtime_library_returns_a_typed_error() {
    let dir = TempDir::new("melomae-ort-missing");
    let missing = dir.join("libonnxruntime.so");
    let error = init_runtime(&missing).expect_err("a missing library must fail");
    assert!(
        matches!(error, InferenceError::Runtime(_)),
        "a missing library must return a typed Runtime error, got: {error}"
    );
}

#[test]
fn a_library_that_is_not_inference_runtime_returns_a_typed_error() {
    let dir = TempDir::new("melomae-ort-wrong");
    let path = dir.join("libonnxruntime.so");
    std::fs::write(&path, b"not an onnx runtime library").expect("write the fake library");
    let error = init_runtime(&path).expect_err("a wrong library must fail");
    assert!(
        matches!(error, InferenceError::Runtime(_)),
        "a wrong library must return a typed Runtime error, got: {error}"
    );
}

#[test]
fn a_missing_inference_library_from_the_public_entry_returns_a_typed_error() {
    let dir = TempDir::new("melomae-ort-entry");
    let missing = dir.join("libonnxruntime.so");
    let error = match load_onnx_backend(&missing, &dir.join("apollo.onnx")) {
        Ok(_) => panic!("a missing library must fail"),
        Err(error) => error,
    };
    assert!(
        matches!(error, InferenceError::Runtime(_)),
        "the public entry must return a typed Runtime error, got: {error}"
    );
}
