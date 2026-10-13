//! Ctrl-C cancel through the real binary.
//!
//! The test sends a real console control event (CTRL_BREAK_EVENT on Windows, SIGINT on
//! Linux) to the running `restore` child and asserts the contract: exit code 130, no
//! output file, and no temporary file. The signal is sent after the first progress
//! line, so the cancel lands while the model runs.

#[path = "../../melomae-core/tests/common/mod.rs"]
mod common;
#[path = "../../melomae-core/tests/setup/mod.rs"]
mod setup;

use std::env;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use melomae_core::decode::decode_file;
use melomae_core::inference::SAMPLE_RATE;

use common::write_wav_f32;
use setup::TempDir;

/// The repository root, from the CLI crate's manifest directory.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate is two levels below the repository root")
        .to_path_buf()
}

/// The committed task 01 Apollo ONNX model.
fn model_path() -> PathBuf {
    repo_root().join("models").join("apollo.onnx")
}

/// The CLI binary under test.
fn cli() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_melomae-cli"))
}

/// Stop with `git lfs pull` guidance when a committed file is missing or is an LFS pointer.
fn require_lfs(path: &Path) {
    let bytes = std::fs::read(path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}). Run: git lfs pull",
            path.display()
        )
    });
    assert!(
        !bytes.starts_with(b"version https://git-lfs.github.com/spec/v1"),
        "{} is a Git LFS pointer, not the data. Run: git lfs pull",
        path.display()
    );
}

/// Build three copies of the committed 6 s parity input as one 18 s WAV.
fn build_tiled_input(dir: &TempDir, tiles: usize, name: &str) -> PathBuf {
    let source_path = repo_root()
        .join("tools")
        .join("parity")
        .join("input_wav.wav");
    let source = decode_file(&source_path).expect("decode the committed input WAV");
    let tiled: Vec<Vec<f32>> = source
        .channels()
        .iter()
        .map(|channel| {
            let mut out = Vec::with_capacity(channel.len() * tiles);
            for _ in 0..tiles {
                out.extend_from_slice(channel);
            }
            out
        })
        .collect();
    let path = dir.join(name);
    write_wav_f32(&path, SAMPLE_RATE, &tiled);
    path
}

/// Wait for the child to exit, or kill it and return `None` after `timeout`.
fn wait_with_timeout(child: &mut Child, timeout: Duration) -> Option<ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().expect("poll the CLI") {
            return Some(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            return None;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

/// Put the child in its own process group, so a console control event can target it alone.
#[cfg(windows)]
fn spawn_in_new_group(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    /// `CREATE_NEW_PROCESS_GROUP`, from `Win32::System::Threading`.
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    command.creation_flags(CREATE_NEW_PROCESS_GROUP);
}

/// The child's own process group is enough on Linux, where the signal is sent to the pid.
#[cfg(unix)]
fn spawn_in_new_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

/// Send the interrupt the CLI treats as a cancel.
#[cfg(windows)]
fn send_interrupt(child: &Child) {
    use windows_sys::Win32::System::Console::{GenerateConsoleCtrlEvent, CTRL_BREAK_EVENT};
    let sent = unsafe { GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, child.id()) };
    assert_ne!(
        sent, 0,
        "GenerateConsoleCtrlEvent must reach the child's group"
    );
}

/// Send the interrupt the CLI treats as a cancel.
#[cfg(unix)]
fn send_interrupt(child: &Child) {
    let killed = unsafe { libc::kill(child.id() as i32, libc::SIGINT) };
    assert_eq!(killed, 0, "kill(SIGINT) must reach the child");
}

#[test]
fn e2e_restore_cancel_exits_130_and_leaves_no_output() {
    require_lfs(&model_path());
    let dir = TempDir::new("melomae-cli-cancel");
    let input = build_tiled_input(&dir, 3, "input_18s.wav");
    let output = dir.join("restored.flac");

    let mut command = Command::new(cli());
    command
        .arg("restore")
        .arg(&input)
        .arg(&output)
        .args(["--format", "flac", "--device", "cpu"])
        .arg("--model")
        .arg(model_path())
        .arg("--onnxruntime")
        .arg(setup::ort_library())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    spawn_in_new_group(&mut command);

    let mut child = command.spawn().expect("spawn the CLI");
    let stderr = child.stderr.take().expect("the stderr pipe");
    let (sender, receiver) = mpsc::channel::<String>();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if sender.send(line).is_err() {
                break;
            }
        }
    });

    // Skip the preamble and wait for the first chunk line, so the signal lands during
    // inference.
    let deadline = Instant::now() + Duration::from_secs(300);
    let mut first = None;
    while Instant::now() < deadline {
        match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(line) if line.starts_with("chunk ") => {
                first = Some(line);
                break;
            }
            Ok(_) => continue,
            Err(_) => break,
        }
    }
    let first = first.expect("the restore must report a chunk progress line before it finishes");
    assert!(
        first.starts_with("chunk 1/"),
        "the first progress line must report chunk 1: {first}"
    );

    send_interrupt(&child);

    let status = wait_with_timeout(&mut child, Duration::from_secs(120))
        .expect("the CLI must exit after the interrupt");
    let _ = reader.join();

    assert_eq!(status.code(), Some(130), "a cancelled restore exits 130");
    assert!(
        !output.exists(),
        "a cancelled restore leaves no output file"
    );

    let stray: Vec<String> = std::fs::read_dir(dir.path())
        .expect("read the temporary directory")
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name != "input_18s.wav")
        .collect();
    assert!(
        stray.is_empty(),
        "a cancelled restore leaves no temporary file: {stray:?}"
    );
}
