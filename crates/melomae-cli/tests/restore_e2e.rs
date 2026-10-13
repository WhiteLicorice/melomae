//! End-to-end tests for `melomae-cli restore`, through the real binary.
//!
//! The tests reuse the core test builders (`setup` and `common`) so the fixtures are
//! built the same way everywhere: synthesized in Rust, encoded with the pinned ffmpeg,
//! and removed when the test ends. Apollo is the committed task 01 model and ONNX
//! Runtime is the fetched pinned release, so these tests fail, and do not skip, when
//! either is missing.

#[path = "../../melomae-core/tests/common/mod.rs"]
mod common;
#[path = "../../melomae-core/tests/setup/mod.rs"]
mod setup;

use std::env;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use melomae_core::decode::decode_file;
use melomae_core::inference::SAMPLE_RATE;

use common::{max_abs_diff, write_wav_f32};
use setup::TempDir;

/// The SRS §12.1 parity limit on the maximum absolute difference.
const MAX_ABS_LIMIT: f32 = 1e-3;

/// The SRS §12.1 parity limit on the root-mean-square error.
const RMSE_LIMIT: f64 = 1e-4;

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

/// Hold the one heavy restore at a time.
///
/// A restore uses several GB, so the parallel test threads would otherwise exhaust the
/// machine's memory and make the chunk chooser reject a chunk that fits on an idle
/// machine. The lock keeps one model run at a time.
fn engine_lock() -> MutexGuard<'static, ()> {
    static ENGINE: Mutex<()> = Mutex::new(());
    ENGINE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The peak working set of another process, in bytes.
///
/// This is `PeakWorkingSetSize`, the counter task 05 read in-process, read through
/// `K32GetProcessMemoryInfo` for the child. A failed read returns 0, so the caller keeps
/// the highest value it saw.
#[cfg(windows)]
fn process_peak_working_set_bytes(pid: u32) -> u64 {
    use std::mem;

    #[repr(C)]
    #[allow(dead_code, non_snake_case)]
    struct ProcessMemoryCounters {
        cb: u32,
        PageFaultCount: u32,
        PeakWorkingSetSize: usize,
        WorkingSetSize: usize,
        QuotaPeakPagedPoolUsage: usize,
        QuotaPagedPoolUsage: usize,
        QuotaPeakNonPagedPoolUsage: usize,
        QuotaNonPagedPoolUsage: usize,
        PagefileUsage: usize,
        PeakPagefileUsage: usize,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut core::ffi::c_void;
        fn K32GetProcessMemoryInfo(
            process: *mut core::ffi::c_void,
            counters: *mut ProcessMemoryCounters,
            size: u32,
        ) -> i32;
        fn CloseHandle(handle: *mut core::ffi::c_void) -> i32;
    }

    /// `PROCESS_QUERY_LIMITED_INFORMATION` and `PROCESS_VM_READ`, the access
    /// `GetProcessMemoryInfo` needs.
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const PROCESS_VM_READ: u32 = 0x0010;

    let handle =
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ, 0, pid) };
    if handle.is_null() {
        return 0;
    }
    let mut counters: ProcessMemoryCounters = unsafe { mem::zeroed() };
    counters.cb = mem::size_of::<ProcessMemoryCounters>() as u32;
    let ok = unsafe { K32GetProcessMemoryInfo(handle, &mut counters, counters.cb) };
    unsafe { CloseHandle(handle) };
    if ok == 0 {
        0
    } else {
        counters.PeakWorkingSetSize as u64
    }
}

/// See the Windows branch. The peak resident set of the child, in bytes.
#[cfg(not(windows))]
fn process_peak_working_set_bytes(pid: u32) -> u64 {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).unwrap_or_default();
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmHWM:") {
            if let Ok(kib) = rest.split_whitespace().next().unwrap_or("0").parse::<u64>() {
                return kib * 1024;
            }
        }
    }
    0
}

/// The first integer after `marker` in `text`.
fn number_after(text: &str, marker: &str) -> Option<u64> {
    let rest = &text[text.find(marker)? + marker.len()..];
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// Run `melomae-cli restore` with explicit model and runtime paths.
fn run_restore(input: &Path, output: &Path, format: &str) -> Output {
    Command::new(cli())
        .arg("restore")
        .arg(input)
        .arg(output)
        .args(["--format", format, "--device", "cpu"])
        .arg("--model")
        .arg(model_path())
        .arg("--onnxruntime")
        .arg(setup::ort_library())
        .output()
        .expect("run the CLI")
}

/// Assert that a CLI run succeeded, with its stderr in the failure message.
fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "the restore failed with {}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Build `tiles` copies of the committed 6 s parity input as one WAV in the temp dir.
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

/// One parsed progress line.
#[derive(Debug, PartialEq, Eq)]
struct Progress {
    chunk: usize,
    chunks: usize,
    channel: usize,
    channels: usize,
}

/// Parse a `chunk n/m channel c/C elapsed S.Ss` line.
fn parse_progress(line: &str) -> Progress {
    let parts: Vec<&str> = line.split_whitespace().collect();
    assert_eq!(parts[0], "chunk", "progress line: {line}");
    assert_eq!(parts[2], "channel", "progress line: {line}");
    assert_eq!(parts[4], "elapsed", "progress line: {line}");
    assert!(parts[5].ends_with('s'), "progress line: {line}");
    let (chunk, chunks) = split_pair(parts[1]);
    let (channel, channels) = split_pair(parts[3]);
    Progress {
        chunk,
        chunks,
        channel,
        channels,
    }
}

/// Split a `a/b` progress token.
fn split_pair(token: &str) -> (usize, usize) {
    let (a, b) = token.split_once('/').expect("a/b progress token");
    (
        a.parse().expect("the numerator is a number"),
        b.parse().expect("the denominator is a number"),
    )
}

/// Test-first plan item 1: every task 03 fixture restores, and each output decodes.
#[test]
fn e2e_restore_each_task03_fixture_decodes() {
    require_lfs(&model_path());
    let _engine = engine_lock();
    let fixtures = setup::Fixtures::build();
    for (name, _rate, channels) in setup::INPUTS {
        let input = fixtures.path(name);
        let output = fixtures.path(&format!("restored-{name}.flac"));
        let result = run_restore(&input, &output, "flac");
        assert_success(&result);
        let decoded = decode_file(&output)
            .unwrap_or_else(|error| panic!("decode the restored {name}: {error}"));
        assert_eq!(decoded.sample_rate(), SAMPLE_RATE, "{name}");
        assert_eq!(decoded.channel_count(), *channels, "{name}");
        assert!(decoded.frames() > 0, "{name}: the output must hold audio");
    }
}

/// Test-first plan item 2: the 44.1 kHz WAV output matches the task 04 golden within
/// the §12.1 tolerances.
#[test]
fn e2e_restore_wav_matches_golden() {
    let input_path = repo_root()
        .join("tools")
        .join("parity")
        .join("input_wav.wav");
    let golden_path = repo_root().join("tools").join("parity").join("golden.wav");
    require_lfs(&input_path);
    require_lfs(&golden_path);
    require_lfs(&model_path());

    let _engine = engine_lock();
    let dir = TempDir::new("melomae-cli-golden");
    let input = build_tiled_input(&dir, 3, "input_18s.wav");
    let output = dir.join("restored.wav");

    let result = run_restore(&input, &output, "wav");
    assert_success(&result);

    let restored = decode_file(&output).expect("decode the restored WAV");
    let golden = decode_file(&golden_path).expect("decode the golden");
    assert_eq!(restored.sample_rate(), SAMPLE_RATE);
    assert_eq!(restored.channel_count(), golden.channel_count());
    for (channel, (got, want)) in restored
        .channels()
        .iter()
        .zip(golden.channels())
        .enumerate()
    {
        assert_eq!(got.len(), want.len(), "channel {channel} length");
        let max_abs = max_abs_diff(got, want);
        let error = rmse(got, want);
        println!(
            "channel {channel}: max abs {max_abs:.6e}, RMSE {error:.6e}, \
             limits {MAX_ABS_LIMIT:.0e} and {RMSE_LIMIT:.0e}"
        );
        assert!(
            max_abs <= MAX_ABS_LIMIT,
            "channel {channel} max abs {max_abs:e} exceeds {MAX_ABS_LIMIT:e}"
        );
        assert!(
            error <= RMSE_LIMIT,
            "channel {channel} RMSE {error:e} exceeds {RMSE_LIMIT:e}"
        );
    }
}

/// Scope item 2: one progress line per chunk per channel, on stderr, naming the chunk
/// index, the chunk count, and the elapsed time.
///
/// The input is 36 s, longer than the 30 s maximum chunk (task 05 scope 4), so the run
/// always has more than one chunk and the per-chunk reporting is exercised whatever
/// memory the machine has.
#[test]
fn e2e_restore_reports_one_progress_line_per_chunk() {
    require_lfs(&model_path());
    let _engine = engine_lock();
    let dir = TempDir::new("melomae-cli-progress");
    let input = build_tiled_input(&dir, 6, "input_36s.wav");
    let output = dir.join("restored.flac");

    let result = run_restore(&input, &output, "flac");
    assert_success(&result);

    let stderr = String::from_utf8_lossy(&result.stderr);
    let lines: Vec<&str> = stderr
        .lines()
        .filter(|line| line.starts_with("chunk "))
        .collect();
    assert!(
        !lines.is_empty(),
        "the restore must report progress: {stderr}"
    );

    let first = parse_progress(lines[0]);
    assert!(
        first.chunks > 1,
        "the fixture must take more than one chunk: {first:?}"
    );
    assert_eq!(first.channels, 2, "the input is stereo");
    assert_eq!(
        lines.len(),
        first.chunks * first.channels,
        "one line per chunk per channel: {stderr}"
    );
    for channel in 1..=first.channels {
        let seen: Vec<usize> = lines
            .iter()
            .map(|line| parse_progress(line))
            .filter(|progress| progress.channel == channel)
            .map(|progress| progress.chunk)
            .collect();
        assert_eq!(
            seen,
            (1..=first.chunks).collect::<Vec<_>>(),
            "channel {channel} must report each chunk once, in order"
        );
    }
}

/// Acceptance item 1: a four-minute input restores on the CPU.
///
/// It runs the real model for minutes and uses several GB, so it is ignored by default.
/// Run it while holding the gate slot (`cargo test --release -p melomae-cli -- --ignored`).
/// The printed line carries the wall time, the end-to-end RTF, the chosen chunk, the
/// available memory, the predicted memory, and the process peak, read the way task 05
/// read it (`PeakWorkingSetSize`).
#[test]
#[ignore = "runs the real model for minutes and several GB; run in the gate slot"]
fn e2e_restore_four_minutes_on_cpu() {
    require_lfs(&model_path());
    let _engine = engine_lock();
    let dir = TempDir::new("melomae-cli-four-minute");
    let input = build_tiled_input(&dir, 40, "input_240s.wav");
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
    let mut child = command.spawn().expect("spawn the CLI");
    let pid = child.id();

    // Sample the child's peak working set until it exits.
    let started = Instant::now();
    let mut peak = 0u64;
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll the CLI") {
            break status;
        }
        peak = peak.max(process_peak_working_set_bytes(pid));
        thread::sleep(Duration::from_millis(200));
    };
    let elapsed = started.elapsed();

    let mut stderr = String::new();
    if let Some(mut pipe) = child.stderr.take() {
        let _ = pipe.read_to_string(&mut stderr);
    }
    assert!(
        status.success(),
        "the four-minute restore failed with {status}\n{stderr}"
    );

    let budget = stderr
        .lines()
        .find(|line| line.contains("chunk budget"))
        .unwrap_or_default();
    let chunk = number_after(budget, "chunk budget: chunk ").unwrap_or(0);
    let available = number_after(budget, "available ").unwrap_or(0);
    let predicted = number_after(budget, "total ").unwrap_or(0);
    let audio_seconds = 40.0 * 6.0;
    let error_percent = if predicted == 0 {
        f64::NAN
    } else {
        (peak as f64 - predicted as f64) / predicted as f64 * 100.0
    };
    println!(
        "four-minute restore: wall {:.1}s, audio {audio_seconds}s, end-to-end RTF {:.4}, \
         chunk {chunk}s, available {available} bytes, predicted {predicted} bytes, \
         peak {peak} bytes, error {error_percent:.2}%, SRS §12.2 baseline torch CPU RTF 2.70",
        elapsed.as_secs_f64(),
        elapsed.as_secs_f64() / audio_seconds
    );

    let decoded = decode_file(&output).expect("decode the restored FLAC");
    assert_eq!(decoded.sample_rate(), SAMPLE_RATE);
    assert_eq!(decoded.channel_count(), 2);
    let expected = (f64::from(SAMPLE_RATE) * audio_seconds).round() as usize;
    assert!(
        decoded.frames().abs_diff(expected) <= (SAMPLE_RATE as usize / 1000) * 10,
        "the four-minute output length {} is off {expected}",
        decoded.frames()
    );
}
