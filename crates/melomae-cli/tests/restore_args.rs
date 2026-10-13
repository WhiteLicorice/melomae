//! Command-line contract tests for `melomae-cli restore`.
//!
//! These tests drive the real binary but never open Apollo: every case stops before
//! the model is loaded, so the suite stays fast. Argument errors use clap's usage
//! error, which exits 2. The error cases that the restore entry itself reports exit 1.

#[path = "../../melomae-core/tests/common/mod.rs"]
mod common;
#[path = "../../melomae-core/tests/setup/mod.rs"]
mod setup;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use setup::TempDir;

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

/// Run the CLI with owned arguments and capture its output.
fn run_owned(args: &[String]) -> Output {
    Command::new(cli())
        .args(args)
        .output()
        .expect("run the CLI")
}

/// The sorted file names in a directory.
fn directory_entries(path: &Path) -> Vec<String> {
    let mut entries: Vec<String> = fs::read_dir(path)
        .expect("read the directory")
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    entries.sort();
    entries
}

/// The restore arguments for an input path and an explicit output path, with existing files for
/// the model and runtime, so the output preflight is what stops the run.
fn restore_arguments(input: &Path, output: &Path) -> Vec<String> {
    let existing = an_existing_file();
    vec![
        "restore".to_string(),
        input.display().to_string(),
        output.display().to_string(),
        "--model".to_string(),
        existing.display().to_string(),
        "--onnxruntime".to_string(),
        existing.display().to_string(),
    ]
}

/// Fix round 1, B1: the CLI must refuse an output that resolves to the input.
#[test]
fn restore_rejects_the_output_equal_to_the_input() {
    let dir = TempDir::new("melomae-cli-identity");
    let input = dir.join("input.wav");
    let bytes = b"the input audio bytes".to_vec();
    fs::write(&input, &bytes).expect("write the input");

    // The same path, a "." prefix spelling of it, and (on Windows) a different case.
    let mut spellings = vec![
        input.clone(),
        input.parent().unwrap().join(".").join("input.wav"),
    ];
    #[cfg(windows)]
    spellings.push(input.parent().unwrap().join("INPUT.WAV"));

    for output in spellings {
        let result = run_owned(&restore_arguments(&input, &output));
        assert_eq!(result.status.code(), Some(1), "output {}", output.display());
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(
            stderr.contains("the output is the input"),
            "the message must refuse the identity: {stderr}"
        );
        assert!(
            stderr.to_lowercase().contains("input.wav"),
            "the message must name the path: {stderr}"
        );
        assert_eq!(
            fs::read(&input).expect("read the input"),
            bytes,
            "the input must not change for {}",
            output.display()
        );
    }
    assert_eq!(
        directory_entries(dir.path()),
        vec!["input.wav".to_string()],
        "no temporary file may remain"
    );
}

/// Fix round 1, B1: the CLI must refuse an existing output and leave it unchanged.
#[test]
fn restore_rejects_an_existing_output() {
    let dir = TempDir::new("melomae-cli-output-exists");
    let input = dir.join("input.wav");
    let output = dir.join("output.flac");
    let input_bytes = b"the input audio bytes".to_vec();
    let output_bytes = b"KEEP THIS OUTPUT".to_vec();
    fs::write(&input, &input_bytes).expect("write the input");
    fs::write(&output, &output_bytes).expect("write the output");

    let result = run_owned(&restore_arguments(&input, &output));

    assert_eq!(result.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("the output already exists"),
        "the message must refuse an existing output: {stderr}"
    );
    assert!(
        stderr.contains("output.flac"),
        "the message must name the output: {stderr}"
    );
    assert_eq!(
        fs::read(&output).expect("read the output"),
        output_bytes,
        "the existing output must stay byte-identical"
    );
    assert_eq!(
        fs::read(&input).expect("read the input"),
        input_bytes,
        "the input must not change"
    );
    assert_eq!(
        directory_entries(dir.path()),
        vec!["input.wav".to_string(), "output.flac".to_string()],
        "no temporary file may remain"
    );
}

/// Fix round 1, B1: a missing output parent exits 1, names the folder, and creates nothing.
#[test]
fn restore_rejects_a_missing_output_parent() {
    let dir = TempDir::new("melomae-cli-parent");
    let input = dir.join("input.wav");
    let input_bytes = b"the input audio bytes".to_vec();
    fs::write(&input, &input_bytes).expect("write the input");
    let missing = dir.path().join("absent");
    let output = missing.join("output.flac");

    let result = run_owned(&restore_arguments(&input, &output));

    assert_eq!(result.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("the output directory does not exist"),
        "the message must name the missing folder: {stderr}"
    );
    assert!(
        stderr.contains("absent"),
        "the message must name the folder: {stderr}"
    );
    assert!(
        !missing.exists(),
        "the CLI must not create the output folder"
    );
    assert_eq!(
        fs::read(&input).expect("read the input"),
        input_bytes,
        "the input must not change"
    );
}
