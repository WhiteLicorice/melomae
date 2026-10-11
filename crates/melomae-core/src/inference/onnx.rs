//! The ONNX Runtime model.
//!
//! The session loads `apollo.onnx` from a caller-given path. The model input
//! and output are both `(batch, channels, samples)` with dynamic axes, from the
//! task 01 export. This engine feeds one channel at a time.

use std::path::Path;

use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::Tensor;

use super::{InferenceError, Model, MODEL_INPUT, MODEL_OUTPUT};

/// The session options the engine builds a model session with.
///
/// The default matches the task 04 engine. A measurement varies `memory_arena`
/// and `memory_pattern` to find how the ONNX Runtime arena affects the peak.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnnxOptions {
    /// Use the ONNX Runtime CPU memory arena.
    pub memory_arena: bool,
    /// Use the ONNX Runtime memory pattern optimization.
    pub memory_pattern: bool,
}

impl Default for OnnxOptions {
    fn default() -> Self {
        Self {
            memory_arena: true,
            memory_pattern: true,
        }
    }
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
        let mut builder = Session::builder()
            .map_err(model_error)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(model_error)?;
        if !options.memory_arena {
            builder = builder
                .with_execution_providers([ort::ep::CPU::default()
                    .with_arena_allocator(false)
                    .build()])
                .map_err(model_error)?;
        }
        if !options.memory_pattern {
            builder = builder.with_memory_pattern(false).map_err(model_error)?;
        }
        let session = builder.commit_from_file(path).map_err(model_error)?;
        Ok(Self { session })
    }
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
