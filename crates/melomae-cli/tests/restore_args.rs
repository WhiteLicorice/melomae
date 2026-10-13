//! Command-line contract tests for `melomae-cli restore`.
//!
//! These tests drive the real binary but never open Apollo: every case stops before
//! the model is loaded, so the suite stays fast. Argument errors use clap's usage
//! error, which exits 2. The error cases that the restore entry itself reports exit 1.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The CLI binary under test.
fn cli() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_melomae-cli"))
}

/// A path that exists, so a model or runtime check passes and the test reaches the
/// next error. The entry only checks that the path is a file.
fn an_existing_file() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")
}

/// Run the CLI with the given arguments and capture its output.
fn run(args: &[&str]) -> Output {
    Command::new(cli())
        .args(args)
        .output()
        .expect("run the CLI")
}

/// Test-first plan: the restore subcommand must exist and print its options.
#[test]
fn restore_help_lists_the_options() {
    let output = run(&["restore", "--help"]);
    assert!(output.status.success(), "restore --help must succeed");
    let text = String::from_utf8_lossy(&output.stdout);
    for option in ["--format", "--device", "--model", "--onnxruntime"] {
        assert!(text.contains(option), "the help must list {option}: {text}");
    }
    assert!(text.contains("flac"), "flac is the default format: {text}");
}

/// `--device gpu` is task 08. Until then it is a clap usage error, exit code 2.
#[test]
fn restore_rejects_an_unknown_device_with_a_usage_error() {
    let output = run(&["restore", "in.wav", "out.flac", "--device", "gpu"]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "an unknown device must use clap's usage error, exit 2"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("gpu"),
        "the error must name the value: {stderr}"
    );
}

/// An unknown format is a clap usage error, exit code 2.
#[test]
fn restore_rejects_an_unknown_format_with_a_usage_error() {
    let output = run(&["restore", "in.wav", "out.mp3", "--format", "mp3"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("mp3"),
        "the error must name the value: {stderr}"
    );
}

/// Both positional paths are required.
#[test]
fn restore_requires_both_paths() {
    let output = run(&["restore", "only-one.wav"]);
    assert_eq!(output.status.code(), Some(2));
}

/// A missing `--model` file exits 1 and names the path and the flag.
#[test]
fn restore_names_a_missing_model_file() {
    let output = run(&[
        "restore",
        "in.wav",
        "out.flac",
        "--model",
        "no-such-model.onnx",
        "--onnxruntime",
        an_existing_file().to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no-such-model.onnx"), "{stderr}");
    assert!(stderr.contains("--model"), "{stderr}");
}

/// A missing `--onnxruntime` file exits 1 and names the path and the flag.
#[test]
fn restore_names_a_missing_runtime_file() {
    let output = run(&[
        "restore",
        "in.wav",
        "out.flac",
        "--model",
        an_existing_file().to_str().unwrap(),
        "--onnxruntime",
        "no-such-runtime.dll",
    ]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no-such-runtime.dll"), "{stderr}");
    assert!(stderr.contains("--onnxruntime"), "{stderr}");
}

/// With no `--model`, the entry looks beside the executable and, missing the file,
/// exits 1 and names the default name and the flag.
#[test]
fn restore_names_the_default_model_beside_the_executable() {
    let output = run(&[
        "restore",
        "in.wav",
        "out.flac",
        "--onnxruntime",
        an_existing_file().to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("apollo.onnx"), "{stderr}");
    assert!(stderr.contains("--model"), "{stderr}");
}

/// A missing input file exits 1 and names the file. The model and runtime checks pass,
/// so the decode runs and reports the missing input.
#[test]
fn restore_reports_a_missing_input_file() {
    let existing = an_existing_file();
    let output = run(&[
        "restore",
        "no-such-input.wav",
        "out.flac",
        "--model",
        existing.to_str().unwrap(),
        "--onnxruntime",
        existing.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no-such-input.wav"), "{stderr}");
}
