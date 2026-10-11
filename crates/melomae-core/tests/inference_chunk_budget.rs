//! Chunk sizing from the memory available at job start.
//!
//! The end-to-end test feeds the chosen chunk into the public restore engine.
//! The peak-memory test is `#[ignore]`d: it runs the real model and can use
//! about 10 GiB, so it runs alone while the run holds the gate slot.

mod common;
mod setup;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use melomae_core::decode::decode_file;
use melomae_core::inference::chunk_budget::{choose_chunk, choose_chunk_for_machine, PAD_SECONDS};
use melomae_core::inference::{
    load_onnx_backend, load_onnx_backend_with_options, runtime_version, ChunkedInference,
    InferenceError, Model, OnnxOptions, RestoreBackend, RestoreOutcome, RestoreSettings,
    SAMPLE_RATE,
};

use common::{max_abs_diff, sine, write_wav_f32};
use setup::TempDir;

/// One gibibyte, as a binary multiple.
const GIB: u64 = 1024 * 1024 * 1024;

/// The crossfade duration the chooser's chunk pairs with (task 05 scope 4).
const OVERLAP_SECONDS: f64 = 1.0;

/// The Git LFS pointer header. A checked-out pointer starts with this line.
const LFS_POINTER: &[u8] = b"version https://git-lfs.github.com/spec/v1";

/// An identity model, so the restored channels must equal the input.
struct IdentityModel;

impl Model for IdentityModel {
    fn infer(&mut self, samples: &[f32]) -> Result<Vec<f32>, InferenceError> {
        Ok(samples.to_vec())
    }
}

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

/// The peak working set of this process, in bytes.
///
/// On Windows this is `PeakWorkingSetSize` through `K32GetProcessMemoryInfo`,
/// the same counter that task 01 read with `psutil.peak_wset`. On Linux it is
/// `VmHWM` in `/proc/self/status`, the peak resident set.
#[cfg(windows)]
fn peak_working_set_bytes() -> u64 {
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
        fn GetCurrentProcess() -> *mut core::ffi::c_void;
        fn K32GetProcessMemoryInfo(
            process: *mut core::ffi::c_void,
            counters: *mut ProcessMemoryCounters,
            size: u32,
        ) -> i32;
    }

    let mut counters: ProcessMemoryCounters = unsafe { mem::zeroed() };
    counters.cb = mem::size_of::<ProcessMemoryCounters>() as u32;
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) };
    assert!(ok != 0, "K32GetProcessMemoryInfo failed");
    counters.PeakWorkingSetSize as u64
}

/// See the Windows branch. The peak resident set, in bytes.
#[cfg(not(windows))]
fn peak_working_set_bytes() -> u64 {
    let status = fs::read_to_string("/proc/self/status").expect("read /proc/self/status");
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmHWM:") {
            let kib: u64 = rest
                .split_whitespace()
                .next()
                .expect("a VmHWM value")
                .parse()
                .expect("an integer VmHWM value");
            return kib * 1024;
        }
    }
    panic!("no VmHWM line in /proc/self/status");
}

/// Build the 60 s stereo input as ten copies of the committed 6 s input.
fn tiled_60s(source: &[Vec<f32>]) -> Vec<Vec<f32>> {
    source
        .iter()
        .map(|channel| {
            let mut tiled = Vec::with_capacity(channel.len() * 10);
            for _ in 0..10 {
                tiled.extend_from_slice(channel);
            }
            tiled
        })
        .collect()
}

#[test]
fn e2e_chunk_budget_feeds_the_restore_engine() {
    // A 13 s stereo file, longer than the chosen chunk, so the engine chunks.
    let dir = TempDir::new("melomae-chunk-budget-e2e");
    let path = dir.join("stereo.wav");
    let frames = 13 * SAMPLE_RATE as usize;
    let left = sine(SAMPLE_RATE, 440.0, 0.4, frames);
    let right = sine(SAMPLE_RATE, 660.0, 0.4, frames);
    write_wav_f32(&path, SAMPLE_RATE, &[left, right]);
    let audio = decode_file(&path).expect("decode the stereo fixture");
    let channels = audio.channels().to_vec();

    // 8 GiB selects a 5 s chunk, so the 13 s file needs three chunks.
    let budget = choose_chunk(8 * GIB).expect("8 GiB must fit a chunk");
    assert_eq!(budget.chunk_seconds, 5);

    let settings = RestoreSettings {
        chunk_seconds: Some(budget.chunk_seconds as f64),
        overlap_seconds: OVERLAP_SECONDS,
        chunk_pad_seconds: PAD_SECONDS as f64,
    };
    let backend = ChunkedInference::new(IdentityModel);
    let outcome = backend
        .restore(&channels, &settings, &mut |_| {}, &AtomicBool::new(false))
        .expect("the restore must succeed");

    match outcome {
        RestoreOutcome::Complete(restored) => {
            assert_eq!(restored.len(), channels.len());
            for (got, want) in restored.iter().zip(&channels) {
                assert!(
                    max_abs_diff(got, want) <= 1e-6,
                    "the identity model must reproduce the input"
                );
            }
        }
        RestoreOutcome::Cancelled => panic!("the run must not cancel"),
    }
}

#[test]
#[ignore = "runs the real model and uses about 10 GiB; run it alone while holding the gate slot"]
fn chunk_budget_60s_restore_peaks_within_15_percent_of_the_prediction() {
    let input_path = repo_root()
        .join("tools")
        .join("parity")
        .join("input_wav.wav");
    let model_path = env::var_os("MELOMAE_APOLLO_ONNX")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root().join("models").join("apollo.onnx"));
    // Fail, and do not skip, when a committed file is an LFS pointer.
    committed_bytes(&input_path);
    committed_bytes(&model_path);

    let source = decode_file(&input_path).expect("decode the committed 6 s input");
    let input = tiled_60s(source.channels());
    assert_eq!(
        input[0].len(),
        SAMPLE_RATE as usize * 60,
        "the tiled input must be 60 s"
    );

    // The chunk this machine would pick at job start.
    let budget = choose_chunk_for_machine().expect("the machine must fit a chunk");
    let settings = RestoreSettings {
        chunk_seconds: Some(budget.chunk_seconds as f64),
        overlap_seconds: OVERLAP_SECONDS,
        chunk_pad_seconds: PAD_SECONDS as f64,
    };

    let backend =
        load_onnx_backend(setup::ort_library(), &model_path).expect("load the Apollo ONNX model");
    let outcome = backend
        .restore(&input, &settings, &mut |_| {}, &AtomicBool::new(false))
        .expect("run the restore");
    assert!(
        matches!(outcome, RestoreOutcome::Complete(_)),
        "the run must not cancel"
    );

    let measured = peak_working_set_bytes();
    let predicted = budget.predicted_peak_bytes;
    let error_percent = (measured as f64 - predicted as f64) / predicted as f64 * 100.0;
    println!(
        "chunk {}s, available {} bytes, budget {} bytes, predicted peak {} bytes, \
         measured peak {} bytes, error {error_percent:.2}%",
        budget.chunk_seconds, budget.available_bytes, budget.budget_bytes, predicted, measured,
    );
    assert!(
        error_percent.abs() <= 15.0,
        "the measured peak is off by {error_percent:.2}% (limit 15%)"
    );
}

/// Read an environment variable as a boolean flag: "1" or "true".
fn flag(name: &str) -> bool {
    matches!(std::env::var(name).as_deref(), Ok("1") | Ok("true"))
}

/// Read an environment variable as an integer, with a default.
fn number(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// The first channel only, as a one-element channel list.
fn first_channel(source: &[Vec<f32>]) -> Vec<Vec<f32>> {
    vec![source[0].clone()]
}

/// A measurement that prints one machine-readable line.
///
/// Environment variables select the configuration, so each run is a fresh
/// process:
///
/// - `MELOMAE_PROBE_CHUNK`: chunk seconds (default 17).
/// - `MELOMAE_PROBE_MONO`: `1` uses one channel of the 60 s input.
/// - `MELOMAE_PROBE_NO_ARENA`: `1` turns the CPU arena and memory patterns off.
/// - `MELOMAE_PROBE_RTF`: `1` times the 6 s parity input instead.
///
/// It is `#[ignore]`d because every configuration can use more than 10 GiB.
#[test]
#[ignore = "measurement: run once per configuration while holding the gate slot"]
fn chunk_budget_peak_probe() {
    let model_path = env::var_os("MELOMAE_APOLLO_ONNX")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root().join("models").join("apollo.onnx"));
    committed_bytes(&model_path);

    let no_arena = flag("MELOMAE_PROBE_NO_ARENA");
    let options = OnnxOptions {
        memory_arena: !no_arena,
        memory_pattern: !no_arena,
    };
    let backend = load_onnx_backend_with_options(setup::ort_library(), &model_path, options)
        .expect("load the Apollo ONNX model");

    let input_path = repo_root()
        .join("tools")
        .join("parity")
        .join("input_wav.wav");

    if flag("MELOMAE_PROBE_RTF") {
        let source = decode_file(&input_path).expect("decode the committed 6 s input");
        let input = source.channels().to_vec();
        let settings = RestoreSettings {
            chunk_seconds: Some(6.0),
            overlap_seconds: OVERLAP_SECONDS,
            chunk_pad_seconds: PAD_SECONDS as f64,
        };
        let start = Instant::now();
        let outcome = backend
            .restore(&input, &settings, &mut |_| {}, &AtomicBool::new(false))
            .expect("run the restore");
        let elapsed = start.elapsed().as_secs_f64();
        assert!(matches!(outcome, RestoreOutcome::Complete(_)));
        let seconds = input[0].len() as f64 / f64::from(SAMPLE_RATE);
        println!(
            "PROBE rtf version={:?} channels={} no_arena={no_arena} elapsed={elapsed:.3} \
             seconds={seconds:.3} rtf={:.4}",
            runtime_version(setup::ort_library()),
            input.len(),
            elapsed / seconds,
        );
        return;
    }

    let source = decode_file(&input_path).expect("decode the committed 6 s input");
    let tiled = tiled_60s(source.channels());
    let is_mono = flag("MELOMAE_PROBE_MONO");
    let input = if is_mono {
        first_channel(&tiled)
    } else {
        tiled
    };
    let chunk = number("MELOMAE_PROBE_CHUNK", 17);

    let settings = RestoreSettings {
        chunk_seconds: Some(chunk as f64),
        overlap_seconds: OVERLAP_SECONDS,
        chunk_pad_seconds: PAD_SECONDS as f64,
    };
    // The peak after the first chunk of the first channel. The callback runs
    // before each chunk, so chunk index 1 is after chunk 0 finished.
    let mut after_first_chunk = 0u64;
    let start = Instant::now();
    let outcome = backend
        .restore(
            &input,
            &settings,
            &mut |progress| {
                if progress.chunk == 1 && progress.channel == 0 && after_first_chunk == 0 {
                    after_first_chunk = peak_working_set_bytes();
                }
            },
            &AtomicBool::new(false),
        )
        .expect("run the restore");
    let elapsed = start.elapsed().as_secs_f64();
    assert!(matches!(outcome, RestoreOutcome::Complete(_)));
    let peak = peak_working_set_bytes();
    println!(
        "PROBE peak version={:?} chunk={chunk} channels={} mono={is_mono} no_arena={no_arena} \
         after_first_chunk={after_first_chunk} peak={peak} elapsed={elapsed:.1}",
        runtime_version(setup::ort_library()),
        input.len(),
    );
}
