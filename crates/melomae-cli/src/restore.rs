//! The `restore` subcommand.
//!
//! It chains the four core stages (task 07 scope 1): decode to 44.1 kHz float audio
//! (task 03), choose the chunk from the free memory (task 05), run Apollo through ONNX
//! Runtime (task 04), and encode the result (task 06). It reports one progress line per
//! chunk on stderr (scope 2), and it turns a cancel into exit code 130 (scope 3) without
//! writing an output file (scope 4).
//!
//! The Apollo model and the ONNX Runtime library are caller-given. Each defaults to a
//! file beside the executable and can be overridden with `--model` and `--onnxruntime`.
//! The app's own resource wiring is tasks 17, 27, and 28.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use clap::{Args, ValueEnum};
use melomae_core::decode::{decode_file, AudioBuffer};
use melomae_core::encode::{encode_with_cancel, EncodeResult, OutputFormat as EncodeFormat};
use melomae_core::inference::chunk_budget::{
    choose_chunk_for_machine, choose_gpu_chunk, FileShape,
};
use melomae_core::inference::{
    gpu_provider_available, load_onnx_backend, load_onnx_backend_on_device, restore_on_device,
    BackendOpener, Device as CoreDevice, Fallback, InferenceError, RestoreBackend, RestoreOutcome,
    RestoreProgress, RestoreSettings,
};

/// The output format of a restore (SRS §7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Format {
    /// FLAC, 24-bit, 44.1 kHz. This is the default.
    Flac,
    /// WAV, 32-bit float, 44.1 kHz.
    Wav,
    /// Opus in Ogg, 48 kHz, 256 kb/s VBR.
    Opus,
}

impl Format {
    /// The core output format.
    fn as_encode(self) -> EncodeFormat {
        match self {
            Self::Flac => EncodeFormat::Flac,
            Self::Wav => EncodeFormat::Wav,
            Self::Opus => EncodeFormat::Opus,
        }
    }

    /// The lower-case name for a message.
    fn label(self) -> &'static str {
        match self {
            Self::Flac => "flac",
            Self::Wav => "wav",
            Self::Opus => "opus",
        }
    }
}

/// The execution device (SRS §6.9).
///
/// `auto` chooses the GPU when a GPU execution provider can start, and the CPU otherwise. `gpu`
/// requests the GPU and falls back to the CPU, with a message, when the GPU path fails to start
/// or runs out of memory (SRS §6.3). `cpu` never opens the GPU.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Device {
    /// Choose the device automatically.
    Auto,
    /// Run on the CPU.
    Cpu,
    /// Run on the GPU, or fall back to the CPU.
    Gpu,
}

impl Device {
    /// The lower-case name for a message.
    fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Cpu => "cpu",
            Self::Gpu => "gpu",
        }
    }

    /// The core device value.
    fn as_core(self) -> CoreDevice {
        match self {
            Self::Auto => CoreDevice::Auto,
            Self::Cpu => CoreDevice::Cpu,
            Self::Gpu => CoreDevice::Gpu,
        }
    }
}

/// Opens the CPU restore backend, and the CUDA one when a provider is available.
///
/// Both devices load `apollo.onnx` through the caller-given ONNX Runtime library. The GPU path
/// registers the CUDA execution provider (task 08). It needs the CUDA and cuDNN libraries, which
/// the GPU ONNX Runtime build keeps beside itself, so the folder of the library is the folder the
/// engine preloads. When the provider is not there, or fails to start, `open` reports
/// [`InferenceError::ProviderStart`], so the caller falls back to the CPU (SRS §6.3).
struct OnnxBackends {
    /// The ONNX Runtime shared library.
    runtime: PathBuf,
    /// The Apollo ONNX model.
    model: PathBuf,
}

impl OnnxBackends {
    /// The folder that holds the CUDA and cuDNN libraries, or `None` when the library has no
    /// parent directory.
    ///
    /// The GPU ONNX Runtime build keeps `onnxruntime_providers_cuda.dll` beside `onnxruntime.dll`,
    /// and `tools/fetch_cuda.py` (and, later, the app's own download) puts the CUDA and cuDNN
    /// libraries in the same folder. So the parent of the runtime library is the folder to preload.
    fn cuda_folder(&self) -> Option<&Path> {
        self.runtime.parent()
    }
}

impl BackendOpener for OnnxBackends {
    fn gpu_available(&self) -> bool {
        self.cuda_folder()
            .is_some_and(|folder| gpu_provider_available(&self.runtime, folder))
    }

    fn open(&self, device: CoreDevice) -> Result<Box<dyn RestoreBackend>, InferenceError> {
        match device {
            CoreDevice::Cpu | CoreDevice::Auto => {
                Ok(Box::new(load_onnx_backend(&self.runtime, &self.model)?))
            }
            CoreDevice::Gpu => {
                let available = self
                    .cuda_folder()
                    .is_some_and(|folder| gpu_provider_available(&self.runtime, folder));
                if !available {
                    return Err(InferenceError::ProviderStart(format!(
                        "the CUDA execution provider is not available from the ONNX Runtime \
                         library `{}`",
                        self.runtime.display()
                    )));
                }
                Ok(Box::new(load_onnx_backend_on_device(
                    &self.runtime,
                    &self.model,
                    CoreDevice::Gpu,
                    self.cuda_folder(),
                )?))
            }
        }
    }
}

/// The free VRAM of the execution device, in bytes, or `None` when it cannot be read.
///
/// The VRAM query arrives with the GPU execution provider (task 08). Until then the chooser uses
/// the fixed conservative chunk of `choose_gpu_chunk(None, ..)`.
fn vram_bytes() -> Option<u64> {
    None
}

/// The `restore` subcommand arguments.
#[derive(Debug, Args)]
pub struct RestoreArgs {
    /// The input audio file.
    pub input: PathBuf,
    /// The output file.
    pub output: PathBuf,
    /// The output format.
    #[arg(long, value_enum, default_value = "flac")]
    pub format: Format,
    /// The execution device.
    #[arg(long, value_enum, default_value = "auto")]
    pub device: Device,
    /// The Apollo ONNX model. Defaults to `apollo.onnx` beside the executable.
    #[arg(long, value_name = "PATH")]
    pub model: Option<PathBuf>,
    /// The ONNX Runtime shared library. Defaults to `onnxruntime.dll` on Windows or
    /// `libonnxruntime.so` on Linux, beside the executable.
    #[arg(long, value_name = "PATH")]
    pub onnxruntime: Option<PathBuf>,
}

/// The one cancel flag of the process. The ctrl-c handler sets it, and the engine stops
/// within one chunk when it is set (SRS §6.6).
static CANCELLED: AtomicBool = AtomicBool::new(false);

/// The crossfade duration between chunks (task 05 scope 4).
const OVERLAP_SECONDS: f64 = 1.0;

/// The audio inferred beyond each chunk edge and then discarded (task 05 scope 4).
const CHUNK_PAD_SECONDS: f64 = 1.0;

/// The Apollo model name beside the executable.
const DEFAULT_MODEL_NAME: &str = "apollo.onnx";

/// The ONNX Runtime library name beside the executable.
#[cfg(windows)]
const DEFAULT_RUNTIME_NAME: &str = "onnxruntime.dll";

/// The ONNX Runtime library name beside the executable.
#[cfg(not(windows))]
const DEFAULT_RUNTIME_NAME: &str = "libonnxruntime.so";

/// Run the `restore` subcommand and turn the result into the process exit code.
pub fn run(args: RestoreArgs) -> ExitCode {
    match execute(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Cancelled) => ExitCode::from(130),
        Err(Failure::Message(message)) => {
            eprintln!("melomae-cli: {message}");
            ExitCode::from(1)
        }
    }
}

/// Why a restore did not finish.
enum Failure {
    /// The caller cancelled the run. The process exits 130.
    Cancelled,
    /// The run failed, with a message that names the reason.
    Message(String),
}

/// Run one restore from the parsed arguments.
fn execute(args: &RestoreArgs) -> Result<(), Failure> {
    // Install the handler first, so a cancel is honored during the decode and the model load too.
    install_cancel_handler().map_err(Failure::Message)?;

    // Reject an output the CLI must not write before any input is read or model is loaded.
    preflight_output(&args.input, &args.output).map_err(Failure::Message)?;

    let model = resolve(
        args.model.as_deref(),
        DEFAULT_MODEL_NAME,
        "--model",
        "the Apollo model",
    )
    .map_err(Failure::Message)?;
    let runtime = resolve(
        args.onnxruntime.as_deref(),
        DEFAULT_RUNTIME_NAME,
        "--onnxruntime",
        "the ONNX Runtime library",
    )
    .map_err(Failure::Message)?;

    // Decode first, so a bad input reports before the model is loaded.
    let audio = decode_file(&args.input).map_err(|error| stage_error(error.to_string()))?;
    check_cancelled()?;

    let shape = FileShape {
        frames: audio.frames() as u64,
        channels: audio.channel_count() as u64,
    };

    eprintln!(
        "melomae-cli: {} -> {} (format {}, device {})",
        args.input.display(),
        args.output.display(),
        args.format.label(),
        args.device.label()
    );

    // The chunk depends on the device that runs, because a GPU run sizes from the VRAM (task 08
    // scope 5) and a CPU run sizes from the free system memory (task 05). The chooser prints its
    // numbers when a device is opened, and a fallback chooses the CPU numbers for its own run.
    let opener = OnnxBackends { runtime, model };
    let settings_for = |device: CoreDevice| -> Result<RestoreSettings, InferenceError> {
        let budget = match device {
            CoreDevice::Gpu => choose_gpu_chunk(vram_bytes(), shape)?,
            _ => choose_chunk_for_machine(shape)?,
        };
        eprintln!("melomae-cli: chunk budget: {budget}");
        Ok(RestoreSettings {
            chunk_seconds: Some(budget.chunk_seconds as f64),
            overlap_seconds: OVERLAP_SECONDS,
            chunk_pad_seconds: CHUNK_PAD_SECONDS,
        })
    };

    let channels = audio.channel_count();
    let started = Instant::now();
    let mut progress = |report: RestoreProgress| {
        // One line per temporal chunk. The channels run one after the other, so the chunk is
        // reported when its last channel starts.
        if report.channel + 1 == channels {
            eprintln!(
                "chunk {}/{} elapsed {:.1}s",
                report.chunk + 1,
                report.chunks,
                started.elapsed().as_secs_f64()
            );
        }
    };
    // SRS §6.3: tell the user when the GPU path is abandoned. This runs before the CPU run, so
    // the message precedes the CPU progress lines.
    let mut on_fallback = |fallback: &Fallback| {
        eprintln!(
            "melomae-cli: GPU restore failed: {}; falling back to the CPU",
            fallback.reason
        );
    };

    let report = restore_on_device(
        &opener,
        args.device.as_core(),
        audio.channels(),
        &settings_for,
        &mut progress,
        &mut on_fallback,
        &CANCELLED,
    )
    .map_err(|error| Failure::Message(error.to_string()))?;
    check_cancelled()?;

    eprintln!("melomae-cli: ran on device {}", report.device.label());

    // A cancel writes no output file. The encoder only runs on a complete restore, and it is
    // cancel-aware, so a cancel during the encode leaves no target and no temporary file either.
    let restored = match report.outcome {
        RestoreOutcome::Cancelled => return Err(Failure::Cancelled),
        RestoreOutcome::Complete(restored) => AudioBuffer::new(audio.sample_rate(), restored),
    };

    match encode_with_cancel(&restored, &args.output, args.format.as_encode(), &CANCELLED)
        .map_err(|error| Failure::Message(error.to_string()))?
    {
        EncodeResult::Cancelled => Err(Failure::Cancelled),
        EncodeResult::Complete(outcome) => {
            // SRS §7.4: report how many samples the integer format clamped.
            if args.format.as_encode().is_integer() && outcome.clamped > 0 {
                eprintln!("melomae-cli: clamped {} samples", outcome.clamped);
            }
            Ok(())
        }
    }
}

/// Fail the run when the caller has cancelled it.
fn check_cancelled() -> Result<(), Failure> {
    if CANCELLED.load(Ordering::SeqCst) {
        Err(Failure::Cancelled)
    } else {
        Ok(())
    }
}

/// Turn a stage error into a failure: a cancel that landed during the stage is a cancel, not an
/// error, even when the stage reported it as one.
fn stage_error(message: String) -> Failure {
    if CANCELLED.load(Ordering::SeqCst) {
        Failure::Cancelled
    } else {
        Failure::Message(message)
    }
}

/// Reject an output the CLI must not write, before any input is read or model is loaded.
///
/// The output must not resolve to the input, must not already exist, and its parent must be an
/// existing directory. Melomae never replaces a file and never creates the output folder here; the
/// app's skip-or-replace policy is task 17's (SRS §8).
fn preflight_output(input: &Path, output: &Path) -> Result<(), String> {
    if resolves_to_the_input(input, output) {
        return Err(format!(
            "the output is the input: `{}`. Melomae never replaces a file.",
            output.display()
        ));
    }
    if output.exists() {
        return Err(format!(
            "the output already exists: `{}`. Melomae never replaces a file.",
            output.display()
        ));
    }
    let parent = output.parent().unwrap_or_else(|| Path::new(""));
    if !parent.as_os_str().is_empty() && !parent.is_dir() {
        return Err(format!(
            "the output directory does not exist: `{}`.",
            parent.display()
        ));
    }
    Ok(())
}

/// Whether `output` resolves to the same file as `input`.
///
/// The file identity is compared first (a volume and file id on Windows, a device and inode on
/// Linux), then the canonical parent plus file name, case-insensitively on Windows. The second
/// comparison also covers an output path that does not exist yet.
fn resolves_to_the_input(input: &Path, output: &Path) -> bool {
    if let Ok(true) = same_file::is_same_file(input, output) {
        return true;
    }
    match (comparable_path(input), comparable_path(output)) {
        (Some(input), Some(output)) => input == output,
        _ => false,
    }
}

/// The canonical parent directory plus file name, lower-cased on Windows.
fn comparable_path(path: &Path) -> Option<String> {
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let parent = parent.canonicalize().ok()?;
    let name = path.file_name()?;
    let text = parent.join(name).to_string_lossy().into_owned();
    Some(if cfg!(windows) {
        text.to_lowercase()
    } else {
        text
    })
}

/// Install the process cancel handler once.
///
/// The handler sets [`CANCELLED`]. `ctrlc` handles `CTRL_BREAK_EVENT` on Windows: its
/// handler ignores the console control-event code (`ctrlc-3.5.2/src/platform/windows/
/// mod.rs:30`), and `ctrlc` documents that `Ctrl+C` maps to `CTRL_C_EVENT` or
/// `CTRL_BREAK_EVENT` (`src/lib.rs:18`).
fn install_cancel_handler() -> Result<(), String> {
    ctrlc::set_handler(|| CANCELLED.store(true, Ordering::SeqCst))
        .map_err(|error| format!("install the cancel handler: {error}"))
}

/// Resolve a caller-given path, or a default name beside the executable.
fn resolve(
    explicit: Option<&Path>,
    default_name: &str,
    flag: &str,
    what: &str,
) -> Result<PathBuf, String> {
    resolve_in(
        explicit,
        executable_directory().as_deref(),
        default_name,
        flag,
        what,
    )
}

/// Resolve a path against an explicit default directory, so a test can inject the
/// directory instead of the running executable's own.
fn resolve_in(
    explicit: Option<&Path>,
    default_directory: Option<&Path>,
    default_name: &str,
    flag: &str,
    what: &str,
) -> Result<PathBuf, String> {
    let (path, source) = match explicit {
        Some(path) => (path.to_path_buf(), format!("given by `{flag}`")),
        None => {
            let directory = default_directory.ok_or_else(|| {
                format!(
                    "cannot find the directory of the running executable to look for \
                     `{default_name}`; pass `{flag} <path>`"
                )
            })?;
            (
                directory.join(default_name),
                format!("beside the executable, as `{default_name}`"),
            )
        }
    };
    if !path.is_file() {
        return Err(format!(
            "{what} is not a file: `{}` ({source}). Pass `{flag} <path>`.",
            path.display()
        ));
    }
    Ok(path)
}

/// The directory that holds the running executable.
fn executable_directory() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|executable| executable.parent().map(Path::to_path_buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The crate directory, which holds files a test can resolve.
    fn crate_directory() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn every_format_maps_to_its_core_format() {
        assert_eq!(Format::Flac.as_encode(), EncodeFormat::Flac);
        assert_eq!(Format::Wav.as_encode(), EncodeFormat::Wav);
        assert_eq!(Format::Opus.as_encode(), EncodeFormat::Opus);
        assert_eq!(Format::Flac.label(), "flac");
        assert_eq!(Format::Wav.label(), "wav");
        assert_eq!(Format::Opus.label(), "opus");
        assert_eq!(Device::Auto.label(), "auto");
        assert_eq!(Device::Cpu.label(), "cpu");
        assert_eq!(Device::Gpu.label(), "gpu");
        assert_eq!(Device::Auto.as_core(), CoreDevice::Auto);
        assert_eq!(Device::Cpu.as_core(), CoreDevice::Cpu);
        assert_eq!(Device::Gpu.as_core(), CoreDevice::Gpu);
    }

    #[test]
    fn the_opener_reports_no_gpu_and_a_gpu_request_fails_to_start() {
        // The crate directory holds no CUDA libraries, so the provider is not available: `auto`
        // runs the CPU and `gpu` must be a fallback, not a panic (SRS §6.3).
        let opener = OnnxBackends {
            runtime: crate_directory().join("Cargo.toml"),
            model: crate_directory().join("Cargo.toml"),
        };
        assert!(!opener.gpu_available());
        let error = match opener.open(CoreDevice::Gpu) {
            Ok(_) => panic!("a GPU request must fail to start"),
            Err(error) => error,
        };
        assert!(matches!(error, InferenceError::ProviderStart(_)), "{error}");
        assert!(
            error.falls_back_to_cpu(),
            "a provider-start failure must trigger the CPU fallback"
        );
    }

    #[test]
    fn an_explicit_path_wins() {
        let existing = crate_directory().join("Cargo.toml");
        let resolved = resolve_in(
            Some(&existing),
            None,
            DEFAULT_MODEL_NAME,
            "--model",
            "the Apollo model",
        )
        .expect("an existing file resolves");
        assert_eq!(resolved, existing);
    }

    #[test]
    fn a_missing_explicit_path_names_the_path_and_the_flag() {
        let missing = crate_directory().join("no-such-model.onnx");
        let error = resolve_in(
            Some(&missing),
            None,
            DEFAULT_MODEL_NAME,
            "--model",
            "the Apollo model",
        )
        .expect_err("a missing file must fail");
        assert!(error.contains("no-such-model.onnx"), "{error}");
        assert!(error.contains("--model"), "{error}");
    }

    #[test]
    fn a_missing_default_names_the_default_and_the_flag() {
        let error = resolve_in(
            None,
            Some(&crate_directory()),
            "no-such-model.onnx",
            "--model",
            "the Apollo model",
        )
        .expect_err("a missing default must fail");
        assert!(error.contains("no-such-model.onnx"), "{error}");
        assert!(error.contains("--model"), "{error}");
    }

    #[test]
    fn a_default_beside_the_executable_resolves_when_it_exists() {
        // `Cargo.toml` stands in for the model: it exists in the crate directory.
        let resolved = resolve_in(
            None,
            Some(&crate_directory()),
            "Cargo.toml",
            "--model",
            "the Apollo model",
        )
        .expect("the default exists");
        assert_eq!(resolved, crate_directory().join("Cargo.toml"));
    }

    #[test]
    fn a_missing_executable_directory_names_the_flag() {
        let error = resolve_in(
            None,
            None,
            DEFAULT_RUNTIME_NAME,
            "--onnxruntime",
            "the ONNX Runtime library",
        )
        .expect_err("no executable directory must fail");
        assert!(error.contains("--onnxruntime"), "{error}");
    }

    #[test]
    fn the_default_runtime_name_matches_the_platform() {
        if cfg!(windows) {
            assert_eq!(DEFAULT_RUNTIME_NAME, "onnxruntime.dll");
        } else {
            assert_eq!(DEFAULT_RUNTIME_NAME, "libonnxruntime.so");
        }
    }
}
