//! The execution device of a restore, and the CPU fallback (SRS §6.3, §6.9, task 08).
//!
//! The device has three values: [`Device::Auto`], [`Device::Cpu`], and [`Device::Gpu`].
//! [`restore_on_device`] runs the job on the requested device. When the GPU provider fails
//! to start or runs out of memory, it runs the job again on the CPU and reports the
//! fallback to the caller, which tells the user (SRS §6.3).

use std::sync::atomic::AtomicBool;

use super::{InferenceError, RestoreBackend, RestoreOutcome, RestoreProgress, RestoreSettings};

/// The execution device of a restore (SRS §6.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Device {
    /// Choose the device automatically.
    Auto,
    /// Run on the CPU.
    Cpu,
    /// Run on the GPU.
    Gpu,
}

impl Device {
    /// The lower-case name for a message.
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Cpu => "cpu",
            Self::Gpu => "gpu",
        }
    }
}

impl std::fmt::Display for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Builds restore backends for a device, and reports whether a GPU can start.
///
/// The pipeline calls [`open`](BackendOpener::open) with [`Device::Cpu`] or
/// [`Device::Gpu`], never with [`Device::Auto`]. [`gpu_available`](BackendOpener::gpu_available)
/// lets [`restore_on_device`] decide whether `Auto` should try the GPU. It must be
/// safe to call before the first open.
pub trait BackendOpener {
    /// Whether a GPU execution provider can start on this machine.
    fn gpu_available(&self) -> bool;
    /// Open the backend for a device.
    ///
    /// It returns [`InferenceError::ProviderStart`] when the GPU provider is not
    /// present or fails to start, so the caller can fall back to the CPU.
    fn open(&self, device: Device) -> Result<Box<dyn RestoreBackend>, InferenceError>;
}

/// Why a GPU restore fell back to the CPU (SRS §6.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fallback {
    /// The device the caller requested.
    pub requested: Device,
    /// The reason the GPU path was abandoned.
    pub reason: InferenceError,
}

/// The device that ran a restore, and any fallback (SRS §6.3, §6.9).
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceReport {
    /// The restore result.
    pub outcome: RestoreOutcome,
    /// The device that ran the restore.
    pub device: Device,
    /// The fallback, when the GPU path was abandoned.
    pub fallback: Option<Fallback>,
}

/// Run a restore on the requested device, with a CPU fallback (SRS §6.3).
///
/// The function tries the GPU first for [`Device::Gpu`], and for
/// [`Device::Auto`] only when [`BackendOpener::gpu_available`] is true. When the
/// GPU path fails to start, or runs out of memory, the function runs the job
/// again on the CPU, reports the fallback to `on_fallback`, and returns a
/// [`DeviceReport`] whose `device` is [`Device::Cpu`]. Every other error is
/// returned unchanged, and the CPU does not run.
///
/// `settings_for` chooses the settings for the device that will run, so a GPU
/// run can use a GPU-sized chunk and a fallback can use a CPU-sized chunk. It is
/// called at most once per attempt.
pub fn restore_on_device(
    opener: &dyn BackendOpener,
    requested: Device,
    channels: &[Vec<f32>],
    settings_for: &dyn Fn(Device) -> Result<RestoreSettings, InferenceError>,
    progress: &mut dyn FnMut(RestoreProgress),
    on_fallback: &mut dyn FnMut(&Fallback),
    cancel: &AtomicBool,
) -> Result<DeviceReport, InferenceError> {
    let attempt_gpu = match requested {
        Device::Cpu => false,
        Device::Gpu => true,
        Device::Auto => opener.gpu_available(),
    };
    if attempt_gpu {
        match run_on_device(
            opener,
            Device::Gpu,
            channels,
            settings_for,
            progress,
            cancel,
        ) {
            Ok(outcome) => {
                return Ok(DeviceReport {
                    outcome,
                    device: Device::Gpu,
                    fallback: None,
                })
            }
            Err(reason) if reason.falls_back_to_cpu() => {
                let fallback = Fallback { requested, reason };
                on_fallback(&fallback);
                let outcome = run_on_device(
                    opener,
                    Device::Cpu,
                    channels,
                    settings_for,
                    progress,
                    cancel,
                )?;
                return Ok(DeviceReport {
                    outcome,
                    device: Device::Cpu,
                    fallback: Some(fallback),
                });
            }
            Err(error) => return Err(error),
        }
    }
    let outcome = run_on_device(
        opener,
        Device::Cpu,
        channels,
        settings_for,
        progress,
        cancel,
    )?;
    Ok(DeviceReport {
        outcome,
        device: Device::Cpu,
        fallback: None,
    })
}

/// Open a device's backend, choose its settings, and run the restore.
fn run_on_device(
    opener: &dyn BackendOpener,
    device: Device,
    channels: &[Vec<f32>],
    settings_for: &dyn Fn(Device) -> Result<RestoreSettings, InferenceError>,
    progress: &mut dyn FnMut(RestoreProgress),
    cancel: &AtomicBool,
) -> Result<RestoreOutcome, InferenceError> {
    let backend = opener.open(device)?;
    let settings = settings_for(device)?;
    backend.restore(channels, &settings, progress, cancel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::{
        restore_on_device, InferenceError, RestoreOutcome, RestoreProgress, RestoreSettings,
    };
    use std::sync::atomic::AtomicBool;
    use std::sync::Mutex;

    /// A backend that returns a fixed sample value, so a test can tell which backend ran.
    struct MarkerBackend {
        marker: f32,
        run_error: Option<InferenceError>,
    }

    impl RestoreBackend for MarkerBackend {
        fn restore(
            &self,
            channels: &[Vec<f32>],
            _settings: &RestoreSettings,
            _progress: &mut dyn FnMut(RestoreProgress),
            _cancel: &AtomicBool,
        ) -> Result<RestoreOutcome, InferenceError> {
            if let Some(error) = &self.run_error {
                return Err(error.clone());
            }
            Ok(RestoreOutcome::Complete(
                channels
                    .iter()
                    .map(|channel| vec![self.marker; channel.len()])
                    .collect(),
            ))
        }
    }

    /// An opener a test drives. It records every device it is asked to open.
    struct FakeOpener {
        gpu_available: bool,
        cpu_open: Result<f32, InferenceError>,
        gpu_open: Result<f32, InferenceError>,
        gpu_run_error: Option<InferenceError>,
        cpu_run_error: Option<InferenceError>,
        opened: Mutex<Vec<Device>>,
    }

    impl FakeOpener {
        fn new(gpu_available: bool) -> Self {
            Self {
                gpu_available,
                cpu_open: Ok(0.0),
                gpu_open: Ok(1.0),
                gpu_run_error: None,
                cpu_run_error: None,
                opened: Mutex::new(Vec::new()),
            }
        }

        fn opened(&self) -> Vec<Device> {
            self.opened.lock().expect("the opened list").clone()
        }
    }

    impl BackendOpener for FakeOpener {
        fn gpu_available(&self) -> bool {
            self.gpu_available
        }

        fn open(&self, device: Device) -> Result<Box<dyn RestoreBackend>, InferenceError> {
            self.opened.lock().expect("the opened list").push(device);
            let result = match device {
                Device::Auto => unreachable!("the opener is never asked to open Auto"),
                Device::Cpu => self.cpu_open.clone(),
                Device::Gpu => self.gpu_open.clone(),
            };
            match result {
                Ok(marker) => {
                    let run_error = match device {
                        Device::Cpu => self.cpu_run_error.clone(),
                        Device::Gpu => self.gpu_run_error.clone(),
                        Device::Auto => None,
                    };
                    Ok(Box::new(MarkerBackend { marker, run_error }))
                }
                Err(error) => Err(error),
            }
        }
    }

    /// A settings chooser that records the device and returns a distinguishable chunk.
    fn recording_settings(
        seen: &Mutex<Vec<Device>>,
    ) -> impl Fn(Device) -> Result<RestoreSettings, InferenceError> + '_ {
        |device| {
            seen.lock().expect("the seen list").push(device);
            Ok(RestoreSettings {
                chunk_seconds: Some(match device {
                    Device::Gpu => 5.0,
                    _ => 20.0,
                }),
                overlap_seconds: 1.0,
                chunk_pad_seconds: 1.0,
            })
        }
    }

    /// The restored marker value of a report.
    fn marker(report: &DeviceReport) -> f32 {
        match &report.outcome {
            RestoreOutcome::Complete(channels) => channels[0][0],
            RestoreOutcome::Cancelled => panic!("the run must not cancel"),
        }
    }

    /// One channel of samples, so the fakes have something to run on.
    fn audio() -> Vec<Vec<f32>> {
        vec![vec![0.0f32; 1_000]]
    }

    #[test]
    fn a_provider_start_failure_falls_back_to_the_cpu_and_reports_it() {
        let mut opener = FakeOpener::new(true);
        opener.gpu_open = Err(InferenceError::ProviderStart("no GPU provider".to_string()));
        let seen = Mutex::new(Vec::new());
        let notified = Mutex::new(Vec::new());
        let report = restore_on_device(
            &opener,
            Device::Gpu,
            &audio(),
            &recording_settings(&seen),
            &mut |_| {},
            &mut |fallback: &Fallback| {
                notified
                    .lock()
                    .expect("the notified list")
                    .push(fallback.reason.clone());
            },
            &AtomicBool::new(false),
        )
        .expect("the fallback must succeed");
        assert_eq!(report.device, Device::Cpu);
        assert_eq!(marker(&report), 0.0, "the CPU backend must have run");
        let fallback = report.fallback.expect("the fallback must be reported");
        assert_eq!(fallback.requested, Device::Gpu);
        assert!(matches!(fallback.reason, InferenceError::ProviderStart(_)));
        assert_eq!(
            notified.lock().expect("the notified list").len(),
            1,
            "the caller must be told once, before the CPU run"
        );
        assert!(matches!(
            notified.lock().expect("the notified list")[0],
            InferenceError::ProviderStart(_)
        ));
        assert_eq!(opener.opened(), vec![Device::Gpu, Device::Cpu]);
        // The GPU backend never opened, so only the CPU settings are chosen.
        assert_eq!(*seen.lock().unwrap(), vec![Device::Cpu]);
    }

    #[test]
    fn an_out_of_memory_failure_falls_back_to_the_cpu_and_reports_it() {
        let mut opener = FakeOpener::new(true);
        opener.gpu_run_error = Some(InferenceError::OutOfMemory("VRAM full".to_string()));
        let seen = Mutex::new(Vec::new());
        let report = restore_on_device(
            &opener,
            Device::Gpu,
            &audio(),
            &recording_settings(&seen),
            &mut |_| {},
            &mut |_| {},
            &AtomicBool::new(false),
        )
        .expect("the fallback must succeed");
        assert_eq!(report.device, Device::Cpu);
        assert_eq!(marker(&report), 0.0, "the CPU backend must have run");
        let fallback = report.fallback.expect("the fallback must be reported");
        assert!(matches!(fallback.reason, InferenceError::OutOfMemory(_)));
        assert_eq!(opener.opened(), vec![Device::Gpu, Device::Cpu]);
        assert_eq!(*seen.lock().unwrap(), vec![Device::Gpu, Device::Cpu]);
    }

    #[test]
    fn a_gpu_request_with_no_gpu_falls_back_to_the_cpu() {
        let mut opener = FakeOpener::new(false);
        opener.gpu_open = Err(InferenceError::ProviderStart("no GPU".to_string()));
        let seen = Mutex::new(Vec::new());
        let report = restore_on_device(
            &opener,
            Device::Gpu,
            &audio(),
            &recording_settings(&seen),
            &mut |_| {},
            &mut |_| {},
            &AtomicBool::new(false),
        )
        .expect("the fallback must succeed");
        assert_eq!(report.device, Device::Cpu);
        assert_eq!(
            report.fallback.expect("the fallback").requested,
            Device::Gpu
        );
        assert_eq!(opener.opened(), vec![Device::Gpu, Device::Cpu]);
    }

    #[test]
    fn auto_without_a_gpu_runs_on_the_cpu_with_no_fallback() {
        let opener = FakeOpener::new(false);
        let seen = Mutex::new(Vec::new());
        let report = restore_on_device(
            &opener,
            Device::Auto,
            &audio(),
            &recording_settings(&seen),
            &mut |_| {},
            &mut |_| {},
            &AtomicBool::new(false),
        )
        .expect("the run must succeed");
        assert_eq!(report.device, Device::Cpu);
        assert!(report.fallback.is_none(), "Auto must not report a fallback");
        assert_eq!(opener.opened(), vec![Device::Cpu]);
        assert_eq!(*seen.lock().unwrap(), vec![Device::Cpu]);
    }

    #[test]
    fn auto_with_a_gpu_runs_on_the_gpu() {
        let opener = FakeOpener::new(true);
        let seen = Mutex::new(Vec::new());
        let report = restore_on_device(
            &opener,
            Device::Auto,
            &audio(),
            &recording_settings(&seen),
            &mut |_| {},
            &mut |_| {},
            &AtomicBool::new(false),
        )
        .expect("the run must succeed");
        assert_eq!(report.device, Device::Gpu);
        assert_eq!(marker(&report), 1.0, "the GPU backend must have run");
        assert!(report.fallback.is_none());
        assert_eq!(opener.opened(), vec![Device::Gpu]);
    }

    #[test]
    fn a_cpu_request_never_opens_the_gpu() {
        let opener = FakeOpener::new(true);
        let seen = Mutex::new(Vec::new());
        let report = restore_on_device(
            &opener,
            Device::Cpu,
            &audio(),
            &recording_settings(&seen),
            &mut |_| {},
            &mut |_| {},
            &AtomicBool::new(false),
        )
        .expect("the run must succeed");
        assert_eq!(report.device, Device::Cpu);
        assert!(report.fallback.is_none());
        assert_eq!(opener.opened(), vec![Device::Cpu]);
    }

    #[test]
    fn a_non_fallback_error_is_returned_and_the_cpu_is_not_run() {
        let mut opener = FakeOpener::new(true);
        opener.gpu_run_error = Some(InferenceError::Model("boom".to_string()));
        let seen = Mutex::new(Vec::new());
        let error = restore_on_device(
            &opener,
            Device::Gpu,
            &audio(),
            &recording_settings(&seen),
            &mut |_| {},
            &mut |_| {},
            &AtomicBool::new(false),
        )
        .expect_err("a model error must not fall back");
        assert!(matches!(error, InferenceError::Model(_)), "{error}");
        assert_eq!(opener.opened(), vec![Device::Gpu], "the CPU must not run");
    }

    #[test]
    fn a_cpu_request_reports_a_cpu_error_unchanged() {
        let mut opener = FakeOpener::new(false);
        opener.cpu_run_error = Some(InferenceError::OutOfMemory("RAM full".to_string()));
        let seen = Mutex::new(Vec::new());
        let error = restore_on_device(
            &opener,
            Device::Cpu,
            &audio(),
            &recording_settings(&seen),
            &mut |_| {},
            &mut |_| {},
            &AtomicBool::new(false),
        )
        .expect_err("a CPU error must be returned");
        assert!(matches!(error, InferenceError::OutOfMemory(_)), "{error}");
    }
}
