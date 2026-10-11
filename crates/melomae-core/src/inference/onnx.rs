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

/// An ONNX Runtime session over `apollo.onnx`.
pub struct OnnxModel {
    session: Session,
}

impl OnnxModel {
    /// Load the model from a file path.
    ///
    /// The caller passes the resource path of the shipped app.
    pub fn load(path: &Path) -> Result<Self, InferenceError> {
        let session = Session::builder()
            .map_err(model_error)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(model_error)?
            .commit_from_file(path)
            .map_err(model_error)?;
        Ok(Self { session })
    }
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
