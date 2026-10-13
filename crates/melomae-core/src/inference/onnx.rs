//! The ONNX Runtime model.
//!
//! The session loads `apollo.onnx` from a caller-given path. The model input
//! and output are both `(batch, channels, samples)` with dynamic axes, from the
//! task 01 export. This engine feeds one channel at a time.

use std::path::Path;

use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::Tensor;

use super::{Device, InferenceError, Model, MODEL_INPUT, MODEL_OUTPUT};

/// The session options the engine builds a model session with.
///
/// The derived `Default` is `false` for both fields. It turns the ONNX Runtime
/// CPU memory arena off. Task 05 measured the arena growing the peak from about
/// 11 GB after the first chunk to 22 GB after the last on a 60 s input. With
/// the arena off the peak stays at the one-chunk prediction. The memory pattern
/// optimization goes with it, as the measured configuration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OnnxOptions {
    /// Use the ONNX Runtime CPU memory arena.
    pub memory_arena: bool,
    /// Use the ONNX Runtime memory pattern optimization.
    pub memory_pattern: bool,
}

/// An ONNX Runtime session over `apollo.onnx`.
pub struct OnnxModel {
    session: Session,
}

impl OnnxModel {
    /// Load the model from a file path.
    ///
    /// The caller passes the resource path of the shipped app.
    pub fn load(path: &Path) -> Result<Self, InferenceError> {
        Self::load_with_options(path, OnnxOptions::default())
    }

    /// Load the model with explicit session options.
    ///
    /// A measurement uses this to compare the CPU memory arena on and off.
    pub fn load_with_options(path: &Path, options: OnnxOptions) -> Result<Self, InferenceError> {
        Self::load_on_device(path, options, Device::Cpu, None)
    }

    /// Load the model on the device that will run it (task 08).
    ///
    /// [`Device::Cpu`] and [`Device::Auto`] load the model over the CPU execution
    /// provider only. [`Device::Gpu`] preloads the CUDA and cuDNN libraries from
    /// `cuda_folder` and registers the CUDA execution provider ahead of the CPU
    /// one. The CUDA provider is registered with `error_on_failure`, so a provider
    /// that cannot start returns [`InferenceError::ProviderStart`] instead of
    /// falling back silently; the caller then retries on the CPU (SRS §6.3).
    pub fn load_on_device(
        path: &Path,
        options: OnnxOptions,
        device: Device,
        cuda_folder: Option<&Path>,
    ) -> Result<Self, InferenceError> {
        let mut builder = Session::builder()
            .map_err(model_error)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(model_error)?;
        match device {
            Device::Gpu => {
                let folder = cuda_folder.ok_or_else(|| {
                    InferenceError::ProviderStart(
                        "no CUDA library folder is configured for the GPU device".to_string(),
                    )
                })?;
                preload_cuda_dylibs(folder)?;
                builder = builder
                    .with_execution_providers([
                        ort::ep::CUDA::default().build().error_on_failure(),
                        cpu_dispatch(options),
                    ])
                    .map_err(|error| InferenceError::ProviderStart(error.to_string()))?;
            }
            Device::Cpu | Device::Auto => {
                if !options.memory_arena {
                    builder = builder
                        .with_execution_providers([cpu_dispatch(options)])
                        .map_err(model_error)?;
                }
            }
        }
        if !options.memory_pattern {
            builder = builder.with_memory_pattern(false).map_err(model_error)?;
        }
        let session = builder.commit_from_file(path).map_err(model_error)?;
        Ok(Self { session })
    }
}

/// The CPU execution provider with the engine's arena setting.
fn cpu_dispatch(options: OnnxOptions) -> ort::ep::ExecutionProviderDispatch {
    ort::ep::CPU::default()
        .with_arena_allocator(options.memory_arena)
        .build()
}

/// Preload the CUDA and cuDNN libraries the CUDA execution provider needs.
///
/// `folder` holds the CUDA and cuDNN dynamic libraries. The GPU ONNX Runtime build
/// and its `onnxruntime_providers_cuda.dll` sit in the same folder, so the provider
/// finds its dependencies. A library that does not load is a provider-start
/// failure, which the caller turns into a CPU fallback (SRS §6.3).
///
/// The function loads each library with `libloading` and keeps it in the process's
/// loaded-module list. It does not use `ort::ep::cuda::preload_dylibs`, because that
/// function builds an `ort::Error` when a library fails to load, and building an
/// `ort::Error` touches `ort`'s global API, which loads a default ONNX Runtime and
/// panics when no runtime is set up yet.
pub(crate) fn preload_cuda_dylibs(folder: &Path) -> Result<(), InferenceError> {
    for name in ort::ep::cuda::CUDA_DYLIBS
        .iter()
        .chain(ort::ep::cuda::CUDNN_DYLIBS)
    {
        let path = folder.join(name);
        let library = unsafe { libloading::Library::new(path.as_os_str()) }.map_err(|error| {
            InferenceError::ProviderStart(format!(
                "the CUDA library `{}` did not load: {error}",
                path.display()
            ))
        })?;
        // Do not drop the handle: an unloaded library would leave the list again.
        std::mem::forget(library);
    }
    Ok(())
}

/// The version of the ONNX Runtime library at `library`.
///
/// Task 01 measured on 1.30.0. Task 05 records the version its measurements ran
/// on, because a different runtime can change the peak memory. The engine loads
/// ONNX Runtime at run time, so this reads `OrtGetApiBase::GetVersionString` from
/// the given shared library. Loading an already loaded library returns the same
/// module, so the process still holds one runtime.
pub fn runtime_version(library: &Path) -> Option<String> {
    let library = unsafe { libloading::Library::new(library) }.ok()?;
    let base_getter: libloading::Symbol<unsafe extern "C" fn() -> *const ort::sys::OrtApiBase> =
        unsafe { library.get(b"OrtGetApiBase") }.ok()?;
    let base = unsafe { base_getter() };
    if base.is_null() {
        return None;
    }
    let version = unsafe { ((*base).GetVersionString)() };
    if version.is_null() {
        return None;
    }
    Some(
        unsafe { std::ffi::CStr::from_ptr(version) }
            .to_string_lossy()
            .into_owned(),
    )
}

impl Model for OnnxModel {
    fn infer(&mut self, samples: &[f32]) -> Result<Vec<f32>, InferenceError> {
        let expected = [1i64, 1i64, samples.len() as i64];
        let tensor =
            Tensor::from_array((expected.to_vec(), samples.to_vec())).map_err(model_error)?;
        let outputs = self
            .session
            .run(ort::inputs![MODEL_INPUT => tensor])
            .map_err(model_error)?;
        let output = outputs.get(MODEL_OUTPUT).ok_or_else(|| {
            InferenceError::Model(format!("the model has no output named {MODEL_OUTPUT}"))
        })?;
        let (shape, data) = output.try_extract_tensor::<f32>().map_err(model_error)?;
        let dims: Vec<i64> = shape.iter().copied().collect();
        if dims != expected {
            return Err(InferenceError::Model(format!(
                "the model output shape {dims:?} does not match {expected:?}"
            )));
        }
        Ok(data.to_vec())
    }
}

/// Fold any ONNX Runtime error into [`InferenceError::Model`].
fn model_error(error: impl std::fmt::Display) -> InferenceError {
    InferenceError::Model(error.to_string())
}
