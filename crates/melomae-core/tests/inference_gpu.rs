//! The CUDA execution provider and the GPU device (task 08, SRS §6.3, §6.9).
//!
//! The GPU path loads the CUDA ONNX Runtime build and the NVIDIA CUDA and cuDNN
//! libraries that `tools/fetch_cuda.py` fetches into `.cache/cuda/<tag>/lib/`. The
//! tests run when an NVIDIA GPU is present; they skip only when none is, and the
//! skip message says so. When a GPU is present but the cache is missing, they fail
//! and name the fetch command, so a green run means the GPU path really ran.
//!
//! The parity test uses the committed `tools/parity` fixtures: the 18 s input (three
//! copies of the committed 6 s input) and the stock Apollo golden. It compares the
//! CUDA output, the CPU output, and the golden within the SRS §12.1 tolerances.

mod common;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use melomae_core::decode::decode_file;
use melomae_core::inference::{
    gpu_provider_available, load_onnx_backend, load_onnx_backend_on_device, restore_on_device,
    BackendOpener, Device, InferenceError, RestoreBackend, RestoreOutcome, RestoreSettings,
    SAMPLE_RATE,
};

use common::max_abs_diff;

/// The pinned CUDA tag. Keep this in step with `tools/fetch_cuda.py`.
const CUDA_TAG: &str = "v1.28.3-cuda12";

/// The SRS §12.1 parity limit on the maximum absolute difference.
const MAX_ABS_LIMIT: f32 = 1e-3;

/// The SRS §12.1 parity limit on the root-mean-square error.
const RMSE_LIMIT: f64 = 1e-4;

/// The chunk settings the golden run used (task 04 scope 10).
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

/// The Apollo model path, from `MELOMAE_APOLLO_ONNX` or the committed artifact.
fn model_path() -> PathBuf {
    env::var_os("MELOMAE_APOLLO_ONNX")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root().join("models").join("apollo.onnx"))
}

/// Read a committed file, and stop with `git lfs pull` guidance when it is missing or
/// is a Git LFS pointer.
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

/// Whether an NVIDIA GPU is present, through `nvidia-smi`.
///
/// `nvidia-smi` ships with the NVIDIA driver. On a machine without one the command
/// is absent or fails, which is the skip condition of these tests.
fn nvidia_gpu_present() -> bool {
    Command::new("nvidia-smi")
        .arg("-L")
        .output()
        .map(|output| output.status.success() && !output.stdout.is_empty())
        .unwrap_or(false)
}

/// The GPU runtime library and the CUDA library folder, or `None` when no NVIDIA GPU
/// is present.
///
/// When a GPU is present but the pinned CUDA build is not installed, this fails and
/// names the fetch command. The tests therefore never skip on a GPU machine.
fn gpu_context() -> Option<(PathBuf, PathBuf)> {
    if !nvidia_gpu_present() {
        return None;
    }
    let lib_dir = repo_root()
        .join(".cache")
        .join("cuda")
        .join(CUDA_TAG)
        .join("lib");
    let runtime = ["onnxruntime.dll", "libonnxruntime.so"]
        .iter()
        .map(|name| lib_dir.join(name))
        .find(|path| path.is_file())
        .unwrap_or_else(|| {
            panic!(
                "an NVIDIA GPU is present, but the pinned CUDA build is not installed at {}.\n\
                 Run: python tools/fetch_cuda.py",
                lib_dir.display()
            )
        });
    Some((runtime, lib_dir))
}

/// Three copies of the committed 6 s input's frames, per channel: the 18 s golden input.
fn golden_input() -> Vec<Vec<f32>> {
    let path = repo_root()
        .join("tools")
        .join("parity")
        .join("input_wav.wav");
    committed_bytes(&path);
    let source = decode_file(&path).expect("decode the committed 6 s input");
    assert_eq!(source.sample_rate(), SAMPLE_RATE);
    source
        .channels()
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

/// The committed stock-Apollo golden, as planar channels.
fn golden_output() -> Vec<Vec<f32>> {
    let path = repo_root().join("tools").join("parity").join("golden.wav");
    let bytes = committed_bytes(&path);
    read_float_wav(&bytes)
}

/// The rate, channel count, and planar samples of a 32-bit float WAV.
fn read_float_wav(bytes: &[u8]) -> Vec<Vec<f32>> {
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
    assert_eq!(rate, SAMPLE_RATE);
    let data = data.expect("the golden has a data chunk");
    let interleaved: Vec<f32> = data
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();
    common::planar_from_interleaved(&interleaved, channels as usize)
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

/// The chunk settings every run in this file uses.
fn settings() -> RestoreSettings {
    RestoreSettings {
        chunk_seconds: Some(CHUNK_SECONDS),
        overlap_seconds: OVERLAP_SECONDS,
        chunk_pad_seconds: CHUNK_PAD_SECONDS,
    }
}

/// Run a restore and return the channels.
fn restore<B: RestoreBackend>(backend: &B, input: &[Vec<f32>]) -> Vec<Vec<f32>> {
    match backend
        .restore(input, &settings(), &mut |_| {}, &AtomicBool::new(false))
        .expect("the restore must succeed")
    {
        RestoreOutcome::Complete(channels) => channels,
        RestoreOutcome::Cancelled => panic!("the run must not cancel"),
    }
}

/// Assert `got` matches `want` within the SRS §12.1 tolerances.
fn assert_within_srs_12_1(got: &[Vec<f32>], want: &[Vec<f32>], what: &str) {
    assert_eq!(got.len(), want.len(), "{what}: channel count");
    for (channel, (got_channel, want_channel)) in got.iter().zip(want).enumerate() {
        assert_eq!(
            got_channel.len(),
            want_channel.len(),
            "{what}: channel {channel} length"
        );
        let max_abs = max_abs_diff(got_channel, want_channel);
        let channel_rmse = rmse(got_channel, want_channel);
        println!(
            "{what} channel {channel}: max abs {max_abs:.6e}, RMSE {channel_rmse:.6e}, \
             limits {MAX_ABS_LIMIT:.0e} and {RMSE_LIMIT:.0e}"
        );
        assert!(
            max_abs <= MAX_ABS_LIMIT,
            "{what}: channel {channel} max abs {max_abs:e} exceeds {MAX_ABS_LIMIT:e}"
        );
        assert!(
            channel_rmse <= RMSE_LIMIT,
            "{what}: channel {channel} RMSE {channel_rmse:e} exceeds {RMSE_LIMIT:e}"
        );
    }
}

/// An opener over the real engine on the GPU runtime, for [`restore_on_device`].
struct GpuOpener {
    runtime: PathBuf,
    model: PathBuf,
    cuda: PathBuf,
}

impl BackendOpener for GpuOpener {
    fn gpu_available(&self) -> bool {
        gpu_provider_available(&self.runtime, &self.cuda)
    }

    fn open(&self, device: Device) -> Result<Box<dyn RestoreBackend>, InferenceError> {
        load_onnx_backend_on_device(&self.runtime, &self.model, device, Some(&self.cuda))
            .map(|backend| Box::new(backend) as Box<dyn RestoreBackend>)
    }
}

#[test]
fn e2e_gpu_device_starts_the_cuda_provider_and_reports_the_gpu() {
    let Some((runtime, cuda)) = gpu_context() else {
        eprintln!("skipping: no NVIDIA GPU was found on this machine");
        return;
    };
    let model = model_path();
    committed_bytes(&model);
    assert!(
        gpu_provider_available(&runtime, &cuda),
        "the CUDA execution provider must be available from {}",
        runtime.display()
    );

    let opener = GpuOpener {
        runtime,
        model,
        cuda,
    };
    let input = golden_input();
    let settings_for = |_device: Device| Ok(settings());

    let report = restore_on_device(
        &opener,
        Device::Gpu,
        &input,
        &settings_for,
        &mut |_| {},
        &mut |_| {},
        &AtomicBool::new(false),
    )
    .expect("the GPU restore must run");

    assert_eq!(
        report.device,
        Device::Gpu,
        "the device that ran must be the GPU when the CUDA provider starts"
    );
    assert!(
        report.fallback.is_none(),
        "the GPU path must not fall back when the CUDA provider starts"
    );
    match report.outcome {
        RestoreOutcome::Complete(channels) => {
            assert_eq!(channels.len(), input.len());
            assert!(
                channels.iter().all(|c| c.iter().all(|s| s.is_finite())),
                "the GPU output must be finite"
            );
        }
        RestoreOutcome::Cancelled => panic!("the run must not cancel"),
    }
}

#[test]
fn gpu_restore_matches_the_cpu_restore_and_the_golden_within_srs_12_1() {
    let Some((runtime, cuda)) = gpu_context() else {
        eprintln!("skipping: no NVIDIA GPU was found on this machine");
        return;
    };
    let model = model_path();
    committed_bytes(&model);

    let input = golden_input();
    let golden = golden_output();

    // Both sessions load ONNX Runtime from the same GPU build: the CPU session uses
    // the CPU execution provider, the GPU session registers CUDA ahead of it. The
    // engine loads the runtime once per process, so one build must serve both.
    let cpu = load_onnx_backend(&runtime, &model).expect("load the CPU session");
    let gpu = load_onnx_backend_on_device(&runtime, &model, Device::Gpu, Some(&cuda))
        .expect("start the CUDA provider");

    let cpu_output = restore(&cpu, &input);
    let gpu_output = restore(&gpu, &input);

    assert_within_srs_12_1(&gpu_output, &cpu_output, "GPU vs CPU");
    assert_within_srs_12_1(&gpu_output, &golden, "GPU vs golden");
}

/// The peak and final VRAM in use, in MiB, through `nvidia-smi`.
fn vram_used_mib() -> Option<u64> {
    let output = Command::new("nvidia-smi")
        .args(["--query-gpu=memory.used", "--format=csv,noheader,nounits"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()?
        .trim()
        .parse()
        .ok()
}

/// A GPU measurement that prints one machine-readable line (task 08).
///
/// It reports the real-time factor on the committed 6 s input, the peak VRAM in use
/// during the run (sampled through `nvidia-smi`), and the peak against the baseline
/// before the run. It is `#[ignore]`d: it runs the real model on the GPU, so it runs
/// once while the run holds the gate slot.
#[test]
#[ignore = "measurement: run once while holding the gate slot"]
fn gpu_rtf_and_vram_probe() {
    let Some((runtime, cuda)) = gpu_context() else {
        eprintln!("skipping: no NVIDIA GPU was found on this machine");
        return;
    };
    let model = model_path();
    committed_bytes(&model);

    let input = golden_input()[..1].to_vec();
    let seconds = input[0].len() as f64 / f64::from(SAMPLE_RATE);

    let backend = load_onnx_backend_on_device(&runtime, &model, Device::Gpu, Some(&cuda))
        .expect("start the CUDA provider");
    // Warm up, so the first-run allocations do not land in the timed run.
    let _ = restore(&backend, &input);

    let baseline = vram_used_mib().unwrap_or(0);
    let stop = Arc::new(AtomicBool::new(false));
    let sampler = {
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            let mut peak = 0u64;
            while !stop.load(Ordering::Relaxed) {
                if let Some(used) = vram_used_mib() {
                    peak = peak.max(used);
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            peak
        })
    };

    let start = Instant::now();
    let outcome = backend.restore(&input, &settings(), &mut |_| {}, &AtomicBool::new(false));
    let elapsed = start.elapsed().as_secs_f64();
    stop.store(true, Ordering::Relaxed);
    let peak = sampler.join().expect("the sampler thread");
    assert!(
        matches!(outcome, Ok(RestoreOutcome::Complete(_))),
        "the probe run must complete"
    );

    println!(
        "PROBE gpu channels={} seconds={seconds:.3} elapsed={elapsed:.3} rtf={:.4} \
         baseline_vram_mib={baseline} peak_vram_mib={peak}",
        input.len(),
        elapsed / seconds,
    );
}
