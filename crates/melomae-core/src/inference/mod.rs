//! Chunked Apollo inference.
//!
//! This module ports the chunked restore algorithm of the stock Apollo
//! `inference.py` (`C:\Lab\Apollo\inference.py:86-221`). Task 01 exported the
//! model with the STFT and iSTFT inside the graph, so this module holds only
//! the chunking, the overlap-add, and the model call. It holds no DSP.
//!
//! [`RestoreBackend`] is the file-level entry the pipeline calls. It takes one
//! file's 44.1 kHz float channels, the settings, a progress callback, and a
//! cancel flag. It returns the restored channels. [`OnnxBackend`] is the only
//! production implementation. The engine runs one channel at a time to halve
//! peak memory (Stack §C).
//!
//! The engine loads ONNX Runtime at run time from a library path the caller
//! gives. [`init_runtime`] does that once per process, so the engine keeps the
//! glibc 2.35 baseline of SRS §2. [`load_onnx_backend`] loads the model after
//! the runtime is up.

pub mod chunk_budget;
mod onnx;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub use onnx::OnnxModel;

/// The production restore backend: the chunked engine over the ONNX model.
///
/// The engine runs one channel at a time (Stack §C).
pub type OnnxBackend = ChunkedInference<OnnxModel>;

/// The sample rate Apollo expects.
pub const SAMPLE_RATE: u32 = 44_100;

/// The model input tensor name, from the task 01 export.
pub const MODEL_INPUT: &str = "audio";

/// The model output tensor name, from the task 01 export.
pub const MODEL_OUTPUT: &str = "audio_out";

/// Chunk durations in samples, from [`resolve_chunking`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkSizes {
    /// The length of one chunk, in samples.
    pub chunk: usize,
    /// The crossfade length between adjacent chunks, in samples.
    pub overlap: usize,
    /// The audio inferred beyond each chunk edge and then discarded, in samples.
    pub pad: usize,
}

/// The chunk settings of one restore, in seconds.
///
/// [`Default`] matches the stock `inference.py` defaults: overlap one second,
/// no padding, and no chunking.
#[derive(Debug, Clone, PartialEq)]
pub struct RestoreSettings {
    /// The chunk duration in seconds. `None` disables chunking.
    pub chunk_seconds: Option<f64>,
    /// The crossfade duration in seconds.
    pub overlap_seconds: f64,
    /// The pad duration in seconds.
    pub chunk_pad_seconds: f64,
}

impl Default for RestoreSettings {
    fn default() -> Self {
        Self {
            chunk_seconds: None,
            overlap_seconds: 1.0,
            chunk_pad_seconds: 0.0,
        }
    }
}

impl RestoreSettings {
    /// Resolve the settings to samples and validate them.
    pub fn resolve_chunking(&self) -> Result<Option<ChunkSizes>, InferenceError> {
        resolve_chunking(
            self.chunk_seconds,
            self.overlap_seconds,
            self.chunk_pad_seconds,
        )
    }
}

/// One progress report, for one chunk of one channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestoreProgress {
    /// The zero-based index of the chunk.
    pub chunk: usize,
    /// The number of chunks in the channel.
    pub chunks: usize,
    /// The zero-based index of the channel.
    pub channel: usize,
}

/// The result of a whole-file restore.
#[derive(Debug, Clone, PartialEq)]
pub enum RestoreOutcome {
    /// The restored channels, in the input channel order.
    Complete(Vec<Vec<f32>>),
    /// The caller cancelled the run. No channel is returned.
    Cancelled,
}

/// An error from settings validation, the model, or the overlap-add.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InferenceError {
    /// The chunk settings are invalid. The message matches the stock script.
    Settings(String),
    /// The model output is shorter than the segment it was given.
    ShortOutput {
        /// The required output length in samples.
        expected: usize,
        /// The length the model returned.
        got: usize,
    },
    /// The retained output holds a sample that is not finite.
    NonFinite {
        /// The channel of the bad sample.
        channel: usize,
        /// The sample index in the channel.
        index: usize,
    },
    /// The overlap-add left at least one output sample with no weight.
    UncoveredOutput,
    /// Even the smallest chunk does not fit the memory budget.
    InsufficientMemory {
        /// The peak the smallest chunk needs, in bytes.
        needed: u64,
        /// The memory available at job start, in bytes.
        available: u64,
    },
    /// The model failed.
    Model(String),
    /// A file operation failed.
    Io(String),
    /// The ONNX Runtime shared library failed to load.
    Runtime(String),
}

impl std::fmt::Display for InferenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Settings(reason) => write!(f, "{reason}"),
            Self::ShortOutput { expected, got } => write!(
                f,
                "the model output is shorter than the corresponding input segment: \
                 expected at least {expected} samples, got {got}"
            ),
            Self::NonFinite { channel, index } => write!(
                f,
                "channel {channel} has a non-finite sample at index {index}"
            ),
            Self::UncoveredOutput => {
                write!(f, "the chunk overlap-add left uncovered output samples")
            }
            Self::InsufficientMemory { needed, available } => write!(
                f,
                "the smallest chunk needs at least {needed} bytes of memory, but only \
                 {available} bytes are available"
            ),
            Self::Model(reason) => write!(f, "the model failed: {reason}"),
            Self::Io(reason) => write!(f, "{reason}"),
            Self::Runtime(reason) => {
                write!(f, "the ONNX Runtime library failed to load: {reason}")
            }
        }
    }
}

impl std::error::Error for InferenceError {}

/// A model that restores one channel of samples.
///
/// The engine calls this with one chunk of one channel at a time, so a
/// test can substitute a fake. The output must be at least as long as the
/// input.
pub trait Model {
    /// Restore one channel of samples. The output keeps the input length.
    fn infer(&mut self, samples: &[f32]) -> Result<Vec<f32>, InferenceError>;
}

/// The whole-file restore entry the pipeline calls.
///
/// The implementation runs one channel at a time and reports progress for each
/// chunk. It checks `cancel` between chunks. On cancel it returns
/// [`RestoreOutcome::Cancelled`] and no channels.
pub trait RestoreBackend {
    /// Restore one file's 44.1 kHz float channels.
    fn restore(
        &self,
        channels: &[Vec<f32>],
        settings: &RestoreSettings,
        progress: &mut dyn FnMut(RestoreProgress),
        cancel: &AtomicBool,
    ) -> Result<RestoreOutcome, InferenceError>;
}

/// Convert optional chunk seconds to samples, and validate the settings.
///
/// This is a port of `resolve_chunking` (`inference.py:86-104`). `None` means
/// chunking is disabled and the whole file runs in one model call.
pub fn resolve_chunking(
    chunk_seconds: Option<f64>,
    overlap_seconds: f64,
    chunk_pad_seconds: f64,
) -> Result<Option<ChunkSizes>, InferenceError> {
    let Some(chunk_seconds) = chunk_seconds else {
        return Ok(None);
    };
    if !chunk_seconds.is_finite() || chunk_seconds <= 0.0 {
        return Err(InferenceError::Settings(
            "Chunk duration must be a positive finite number.".to_string(),
        ));
    }
    if !overlap_seconds.is_finite() || overlap_seconds < 0.0 {
        return Err(InferenceError::Settings(
            "Overlap duration must be a non-negative finite number.".to_string(),
        ));
    }
    if !chunk_pad_seconds.is_finite() || chunk_pad_seconds < 0.0 {
        return Err(InferenceError::Settings(
            "Chunk pad duration must be a non-negative finite number.".to_string(),
        ));
    }

    // Python's `round` is round-half-to-even, so use `round_ties_even` to keep
    // the sample counts identical to the stock script.
    let chunk = (chunk_seconds * f64::from(SAMPLE_RATE)).round_ties_even() as usize;
    let overlap = (overlap_seconds * f64::from(SAMPLE_RATE)).round_ties_even() as usize;
    let pad = (chunk_pad_seconds * f64::from(SAMPLE_RATE)).round_ties_even() as usize;
    if chunk < 1 {
        return Err(InferenceError::Settings(
            "Chunk duration is shorter than one sample.".to_string(),
        ));
    }
    if overlap * 2 > chunk {
        return Err(InferenceError::Settings(
            "Overlap duration must not exceed half the chunk duration.".to_string(),
        ));
    }
    Ok(Some(ChunkSizes {
        chunk,
        overlap,
        pad,
    }))
}

/// The chunk starts that cover the signal with no redundant final chunk.
///
/// This is a port of `chunk_starts` (`inference.py:118-124`).
pub fn chunk_starts(total: usize, chunk: usize, overlap: usize) -> Vec<usize> {
    let hop = chunk.saturating_sub(overlap);
    let mut starts = vec![0usize];
    while starts[starts.len() - 1] + chunk < total {
        starts.push(starts[starts.len() - 1] + hop);
    }
    starts
}

/// The linear edge weights of one chunk, for normalized overlap-add.
///
/// This is a port of `crossfade_weights` (`inference.py:127-137`). A fade is a
/// linear ramp from 0 to 1 over the overlap, matching `torch.linspace(0, 1, n)`.
pub fn crossfade_weights(length: usize, overlap: usize, fade_in: bool, fade_out: bool) -> Vec<f32> {
    let mut weights = vec![1.0f32; length];
    let fade = overlap.min(length);
    if fade > 0 {
        let ramp = linspace(fade);
        if fade_in {
            weights[..fade].copy_from_slice(&ramp);
        }
        if fade_out {
            for (index, value) in ramp.iter().rev().enumerate() {
                weights[length - fade + index] = *value;
            }
        }
    }
    weights
}

/// A linear ramp from 0 to 1 with `length` samples, matching
/// `torch.linspace(0, 1, length)`. A length of one gives `[0.0]`.
fn linspace(length: usize) -> Vec<f32> {
    if length <= 1 {
        return vec![0.0];
    }
    let divisor = (length - 1) as f32;
    (0..length).map(|index| index as f32 / divisor).collect()
}

/// Run the chunked restore for one channel.
///
/// This is a port of `run_model` (`inference.py:140-221`) with batch size one.
/// It returns `None` when the caller cancelled.
fn run_channel<M: Model>(
    model: &mut M,
    samples: &[f32],
    chunk: Option<ChunkSizes>,
    channel: usize,
    progress: &mut dyn FnMut(usize, usize),
    cancel: &AtomicBool,
) -> Result<Option<Vec<f32>>, InferenceError> {
    let total = samples.len();
    match chunk {
        None => run_full_file(model, samples, channel, progress, cancel),
        Some(sizes) if total <= sizes.chunk => {
            run_full_file(model, samples, channel, progress, cancel)
        }
        Some(sizes) => run_chunked(model, samples, sizes, channel, progress, cancel),
    }
}

/// Return an error for the first non-finite sample in the retained output.
///
/// `start` is the sample index of `samples[0]` in the channel.
fn check_finite(samples: &[f32], start: usize, channel: usize) -> Result<(), InferenceError> {
    for (offset, sample) in samples.iter().enumerate() {
        if !sample.is_finite() {
            return Err(InferenceError::NonFinite {
                channel,
                index: start + offset,
            });
        }
    }
    Ok(())
}

/// The whole-file path: one model call with no padding.
fn run_full_file<M: Model>(
    model: &mut M,
    samples: &[f32],
    channel: usize,
    progress: &mut dyn FnMut(usize, usize),
    cancel: &AtomicBool,
) -> Result<Option<Vec<f32>>, InferenceError> {
    if cancel.load(Ordering::Relaxed) {
        return Ok(None);
    }
    progress(0, 1);
    let output = model.infer(samples)?;
    if cancel.load(Ordering::Relaxed) {
        return Ok(None);
    }
    if output.len() < samples.len() {
        return Err(InferenceError::ShortOutput {
            expected: samples.len(),
            got: output.len(),
        });
    }
    check_finite(&output, 0, channel)?;
    Ok(Some(output))
}

/// The chunked path: padded chunks with normalized overlap-add.
fn run_chunked<M: Model>(
    model: &mut M,
    samples: &[f32],
    sizes: ChunkSizes,
    channel: usize,
    progress: &mut dyn FnMut(usize, usize),
    cancel: &AtomicBool,
) -> Result<Option<Vec<f32>>, InferenceError> {
    let total = samples.len();
    let starts = chunk_starts(total, sizes.chunk, sizes.overlap);
    let chunks = starts.len();
    let padded_chunk = sizes.chunk + 2 * sizes.pad;

    let mut output_sum = vec![0.0f32; total];
    let mut weight_sum = vec![0.0f32; total];

    for (index, start) in starts.iter().copied().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Ok(None);
        }
        progress(index, chunks);

        let end = (start + sizes.chunk).min(total);
        let valid = end - start;
        // Real audio pads each side. The file's own edges take none: the first
        // padded chunk begins at the file start, and the last ends at the file
        // end, extending further left instead.
        let mut padded_start = start.saturating_sub(sizes.pad);
        if sizes.pad > 0 && end == total {
            padded_start = total.saturating_sub(padded_chunk);
        }

        // Build the padded segment. Samples past the file end stay zero, which
        // matches `F.pad`.
        let mut segment = vec![0.0f32; padded_chunk];
        let take = (total - padded_start).min(padded_chunk);
        segment[..take].copy_from_slice(&samples[padded_start..padded_start + take]);

        let output = model.infer(&segment)?;
        if cancel.load(Ordering::Relaxed) {
            return Ok(None);
        }
        if output.len() < padded_chunk {
            return Err(InferenceError::ShortOutput {
                expected: padded_chunk,
                got: output.len(),
            });
        }

        let valid_start = start - padded_start;
        let chunk_output = &output[valid_start..valid_start + valid];
        check_finite(chunk_output, start, channel)?;
        let weights = crossfade_weights(valid, sizes.overlap, index > 0, index < chunks - 1);
        for offset in 0..valid {
            output_sum[start + offset] += chunk_output[offset] * weights[offset];
            weight_sum[start + offset] += weights[offset];
        }
    }

    if weight_sum.iter().any(|weight| *weight <= 0.0) {
        return Err(InferenceError::UncoveredOutput);
    }
    let restored = output_sum
        .iter()
        .zip(&weight_sum)
        .map(|(sum, weight)| sum / weight)
        .collect();
    Ok(Some(restored))
}

/// A [`RestoreBackend`] over any [`Model`].
///
/// The production engine is [`OnnxBackend`], which is this type over
/// [`onnx::OnnxModel`]. Tests build it over a fake model.
pub struct ChunkedInference<M: Model> {
    model: Mutex<M>,
}

impl<M: Model> ChunkedInference<M> {
    /// Wrap a model into a restore backend.
    pub fn new(model: M) -> Self {
        Self {
            model: Mutex::new(model),
        }
    }
}

impl<M: Model> RestoreBackend for ChunkedInference<M> {
    fn restore(
        &self,
        channels: &[Vec<f32>],
        settings: &RestoreSettings,
        progress: &mut dyn FnMut(RestoreProgress),
        cancel: &AtomicBool,
    ) -> Result<RestoreOutcome, InferenceError> {
        let chunk = settings.resolve_chunking()?;
        let mut model = self
            .model
            .lock()
            .map_err(|_| InferenceError::Model("the model lock was poisoned".to_string()))?;

        let mut restored = Vec::with_capacity(channels.len());
        for (channel_index, channel) in channels.iter().enumerate() {
            let mut on_chunk = |chunk_index: usize, chunks: usize| {
                progress(RestoreProgress {
                    chunk: chunk_index,
                    chunks,
                    channel: channel_index,
                });
            };
            match run_channel(
                &mut *model,
                channel,
                chunk,
                channel_index,
                &mut on_chunk,
                cancel,
            )? {
                Some(output) => restored.push(output),
                None => return Ok(RestoreOutcome::Cancelled),
            }
        }
        if cancel.load(Ordering::Relaxed) {
            return Ok(RestoreOutcome::Cancelled);
        }
        Ok(RestoreOutcome::Complete(restored))
    }
}

/// The one-time ONNX Runtime state of this process.
enum RuntimeState {
    /// No load has been attempted.
    NotTried,
    /// The runtime loaded from this resolved library path.
    Loaded(PathBuf),
    /// The runtime failed to load from this resolved path, with the error text.
    Failed {
        /// The resolved path that failed.
        path: PathBuf,
        /// The error text ONNX Runtime reported.
        message: String,
    },
}

/// Initialize the ONNX Runtime once per process from an explicit library path.
///
/// The engine loads ONNX Runtime at run time, so it never links the runtime. The
/// caller gives the shared library: `onnxruntime.dll` on Windows, and
/// `libonnxruntime.so` on Linux. Call this before any other engine call.
///
/// The runtime loads once per process. The function resolves the library to a
/// canonical path and records it. It never unloads or replaces a loaded runtime.
/// A later call with the same resolved path returns `Ok`. A later call with a
/// different path, or with a path that does not exist, returns
/// [`InferenceError::Runtime`] and names the loaded path and the requested path.
///
/// A path that is not a file returns [`InferenceError::Runtime`] before ONNX
/// Runtime is touched, so the caller can correct it. After ONNX Runtime itself
/// fails, the failure is remembered. Every later call returns the same
/// [`InferenceError::Runtime`] and never touches ONNX Runtime again. ONNX Runtime
/// 2.0.0-rc.13 marks its dylib slot as used even when a load fails, so a second
/// load reads uninitialized data and aborts the process. Restart the process to
/// retry the load after a failure.
pub fn init_runtime(library: &Path) -> Result<(), InferenceError> {
    static STATE: Mutex<RuntimeState> = Mutex::new(RuntimeState::NotTried);
    let mut state = STATE
        .lock()
        .map_err(|_| InferenceError::Runtime("the runtime lock was poisoned".to_string()))?;
    match &*state {
        RuntimeState::Loaded(loaded) => {
            if std::fs::canonicalize(library).ok().as_deref() == Some(loaded.as_path()) {
                return Ok(());
            }
            return Err(InferenceError::Runtime(format!(
                "ONNX Runtime is already loaded from `{}`, so the requested library `{}` is not used",
                loaded.display(),
                library.display()
            )));
        }
        RuntimeState::Failed { path, message } => {
            return Err(InferenceError::Runtime(format!(
                "ONNX Runtime failed to load from `{}`: {message}",
                path.display()
            )));
        }
        RuntimeState::NotTried => {}
    }
    if !library.is_file() {
        return Err(InferenceError::Runtime(format!(
            "the ONNX Runtime library does not exist: {}",
            library.display()
        )));
    }
    let resolved = std::fs::canonicalize(library).map_err(|error| {
        InferenceError::Runtime(format!(
            "could not resolve the ONNX Runtime library `{}`: {error}",
            library.display()
        ))
    })?;
    match ort::init_from(&resolved) {
        Ok(builder) => {
            builder.commit();
            *state = RuntimeState::Loaded(resolved);
            Ok(())
        }
        Err(error) => {
            let message = error.to_string();
            let failure = InferenceError::Runtime(format!(
                "ONNX Runtime failed to load from `{}`: {message}",
                resolved.display()
            ));
            *state = RuntimeState::Failed {
                path: resolved,
                message,
            };
            Err(failure)
        }
    }
}

/// Load the Apollo ONNX model from a caller-given runtime library and model path.
///
/// The caller passes the runtime library and the resource path of the shipped app.
/// [`init_runtime`] runs first, so the process loads ONNX Runtime once.
pub fn load_onnx_backend(
    runtime_library: &Path,
    model: &Path,
) -> Result<OnnxBackend, InferenceError> {
    init_runtime(runtime_library)?;
    Ok(ChunkedInference::new(onnx::OnnxModel::load(model)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    /// A model that returns its input unchanged.
    struct IdentityModel;

    impl Model for IdentityModel {
        fn infer(&mut self, samples: &[f32]) -> Result<Vec<f32>, InferenceError> {
            Ok(samples.to_vec())
        }
    }

    /// A model that returns one sample less than its input.
    struct ShortOutputModel;

    impl Model for ShortOutputModel {
        fn infer(&mut self, samples: &[f32]) -> Result<Vec<f32>, InferenceError> {
            Ok(samples[..samples.len() - 1].to_vec())
        }
    }

    /// A model that marks this many samples at each edge as a large finite
    /// value. It mimics the real model, whose output near an input edge is
    /// wrong. The marker is finite, because the engine now rejects a
    /// non-finite retained sample, and the file's own edges are retained.
    struct DegradedEdgesModel {
        degraded: usize,
    }

    impl Model for DegradedEdgesModel {
        fn infer(&mut self, samples: &[f32]) -> Result<Vec<f32>, InferenceError> {
            const MARKER: f32 = 1.0e6;
            let mut output = samples.to_vec();
            let degraded = self.degraded.min(output.len());
            let length = output.len();
            for sample in &mut output[..degraded] {
                *sample = MARKER;
            }
            for sample in &mut output[length - degraded..] {
                *sample = MARKER;
            }
            Ok(output)
        }
    }

    /// A model that records the length of every input it sees.
    struct RecordingModel {
        identity: IdentityModel,
        lengths: Vec<usize>,
    }

    impl Model for RecordingModel {
        fn infer(&mut self, samples: &[f32]) -> Result<Vec<f32>, InferenceError> {
            self.lengths.push(samples.len());
            self.identity.infer(samples)
        }
    }

    /// An identity model that counts its calls, so a test can prove how many
    /// chunks ran. The counter is shared, because the backend owns the model.
    struct CountingModel {
        calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    impl Model for CountingModel {
        fn infer(&mut self, samples: &[f32]) -> Result<Vec<f32>, InferenceError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Ok(samples.to_vec())
        }
    }

    /// A deterministic ramp, so the round-trip is not trivially zero.
    fn ramp(length: usize) -> Vec<f32> {
        (0..length)
            .map(|index| (index as f32 / length as f32) - 0.5)
            .collect()
    }

    /// Run one channel through a backend and return the outcome.
    fn restore_one<M: Model>(
        backend: &ChunkedInference<M>,
        samples: &[f32],
        settings: &RestoreSettings,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(RestoreProgress),
    ) -> RestoreOutcome {
        backend
            .restore(
                std::slice::from_ref(&samples.to_vec()),
                settings,
                progress,
                cancel,
            )
            .expect("the restore must succeed")
    }

    /// Build settings from sample counts, so the test never depends on rounding.
    fn settings(chunk: usize, overlap: usize, pad: usize) -> RestoreSettings {
        let rate = f64::from(SAMPLE_RATE);
        RestoreSettings {
            chunk_seconds: Some(chunk as f64 / rate),
            overlap_seconds: overlap as f64 / rate,
            chunk_pad_seconds: pad as f64 / rate,
        }
    }

    /// Assert that two buffers agree within `tolerance`.
    fn assert_close(a: &[f32], b: &[f32], tolerance: f32) {
        assert_eq!(a.len(), b.len(), "buffers must be the same length");
        for (index, (x, y)) in a.iter().zip(b).enumerate() {
            assert!(
                (x - y).abs() <= tolerance,
                "index {index}: {x} and {y} differ by more than {tolerance}"
            );
        }
    }

    #[test]
    fn an_identity_model_reproduces_the_input() {
        let samples = ramp(9_000);
        let backend = ChunkedInference::new(IdentityModel);
        let outcome = restore_one(
            &backend,
            &samples,
            &settings(3_000, 1_000, 500),
            &AtomicBool::new(false),
            &mut |_| {},
        );
        match outcome {
            RestoreOutcome::Complete(channels) => assert_close(&channels[0], &samples, 1e-6),
            RestoreOutcome::Cancelled => panic!("the run must not cancel"),
        }
    }

    #[test]
    fn padding_wider_than_the_hop_reproduces_the_input() {
        // Regression: `padded_start` must clamp at the file start when the pad
        // exceeds the hop. The port of `inference.py` does that.
        let samples = ramp(80_000);
        let backend = ChunkedInference::new(IdentityModel);
        let outcome = restore_one(
            &backend,
            &samples,
            &settings(22_050, 4_410, 22_050),
            &AtomicBool::new(false),
            &mut |_| {},
        );
        match outcome {
            RestoreOutcome::Complete(channels) => assert_close(&channels[0], &samples, 1e-6),
            RestoreOutcome::Cancelled => panic!("the run must not cancel"),
        }
    }

    #[test]
    fn a_degraded_edges_model_is_discarded_by_the_pad() {
        let degraded = (0.05 * f64::from(SAMPLE_RATE)) as usize;
        let samples = ramp(80_000);
        let backend = ChunkedInference::new(DegradedEdgesModel { degraded });
        let outcome = restore_one(
            &backend,
            &samples,
            &settings(22_050, 4_410, degraded),
            &AtomicBool::new(false),
            &mut |_| {},
        );
        let channels = match outcome {
            RestoreOutcome::Complete(channels) => channels,
            RestoreOutcome::Cancelled => panic!("the run must not cancel"),
        };
        let restored = &channels[0];
        // The file's own first and last degraded samples have nothing outside
        // the file to pad with, so they stay wrong. The check starts after
        // them, as the Python test does.
        let interior = &restored[degraded..restored.len() - degraded];
        assert!(
            interior.iter().all(|sample| *sample < 1.0e6),
            "no degraded marker may reach the interior of the output"
        );
        assert_close(interior, &samples[degraded..samples.len() - degraded], 1e-6);
    }

    #[test]
    fn a_short_model_output_is_an_error() {
        let samples = ramp(100);
        let backend = ChunkedInference::new(ShortOutputModel);
        let error = backend
            .restore(
                std::slice::from_ref(&samples),
                &settings(60, 10, 0),
                &mut |_| {},
                &AtomicBool::new(false),
            )
            .expect_err("a short output must fail");
        assert!(
            error.to_string().contains("shorter"),
            "the error must name the short output, got: {error}"
        );
    }

    #[test]
    fn an_input_no_longer_than_one_chunk_takes_the_full_file_path() {
        let samples = ramp(800);
        let mut model = RecordingModel {
            identity: IdentityModel,
            lengths: Vec::new(),
        };
        let sizes = resolve_chunking(Some(1000.0 / f64::from(SAMPLE_RATE)), 0.0, 0.0)
            .expect("valid settings")
            .expect("chunking is on");
        let result = run_channel(
            &mut model,
            &samples,
            Some(sizes),
            0,
            &mut |_, _| {},
            &AtomicBool::new(false),
        )
        .expect("the run must succeed");
        assert_eq!(model.lengths, vec![800], "one call with the whole file");
        assert_eq!(result, Some(samples));
    }

    #[test]
    fn cancel_during_chunk_two_stops_the_run() {
        let samples = ramp(80_000);
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let backend = ChunkedInference::new(CountingModel {
            calls: std::sync::Arc::clone(&calls),
        });
        let settings = settings(22_050, 4_410, 0);
        let cancel = AtomicBool::new(false);
        let mut seen = Vec::new();
        let outcome = backend
            .restore(
                std::slice::from_ref(&samples),
                &settings,
                &mut |progress| {
                    seen.push(progress.chunk);
                    if progress.chunk == 2 {
                        cancel.store(true, Ordering::Relaxed);
                    }
                },
                &cancel,
            )
            .expect("the restore must not fail");
        assert_eq!(outcome, RestoreOutcome::Cancelled);
        assert_eq!(seen, vec![0, 1, 2], "the run stops during chunk 2");
        assert_eq!(
            calls.load(Ordering::Relaxed),
            3,
            "the model runs once per chunk and stops within one chunk"
        );
    }

    #[test]
    fn progress_reports_chunk_n_of_m_for_each_channel() {
        let channels = vec![ramp(80_000), ramp(80_000)];
        let backend = ChunkedInference::new(IdentityModel);
        let settings = settings(22_050, 4_410, 0);
        let mut seen = Vec::new();
        let outcome = backend
            .restore(
                &channels,
                &settings,
                &mut |progress| seen.push(progress),
                &AtomicBool::new(false),
            )
            .expect("the restore must succeed");
        assert!(matches!(outcome, RestoreOutcome::Complete(_)));
        let chunks = seen.iter().filter(|progress| progress.channel == 0).count();
        assert!(chunks > 1, "the fixture must have more than one chunk");
        for channel in 0..2 {
            let reports: Vec<_> = seen
                .iter()
                .filter(|progress| progress.channel == channel)
                .collect();
            assert_eq!(reports.len(), chunks);
            for (index, progress) in reports.iter().enumerate() {
                assert_eq!(progress.chunk, index);
                assert_eq!(progress.chunks, chunks);
            }
        }
    }

    /// A hand-written fake backend. It proves the pipeline can drive
    /// [`RestoreBackend`] without ONNX Runtime.
    struct FakeBackend;

    impl RestoreBackend for FakeBackend {
        fn restore(
            &self,
            channels: &[Vec<f32>],
            _settings: &RestoreSettings,
            progress: &mut dyn FnMut(RestoreProgress),
            cancel: &AtomicBool,
        ) -> Result<RestoreOutcome, InferenceError> {
            let chunks = channels.first().map_or(0, Vec::len).div_ceil(1_000);
            for (channel, samples) in channels.iter().enumerate() {
                for chunk in 0..chunks {
                    if cancel.load(Ordering::Relaxed) {
                        return Ok(RestoreOutcome::Cancelled);
                    }
                    progress(RestoreProgress {
                        chunk,
                        chunks,
                        channel,
                    });
                    let _ = samples;
                }
            }
            Ok(RestoreOutcome::Complete(channels.to_vec()))
        }
    }

    #[test]
    fn a_fake_restore_backend_drives_one_file_through_the_trait() {
        let channels = vec![vec![0.0f32; 2_500], vec![0.0f32; 2_500]];
        let mut seen = Vec::new();
        let outcome = FakeBackend
            .restore(
                &channels,
                &RestoreSettings::default(),
                &mut |progress| seen.push(progress),
                &AtomicBool::new(false),
            )
            .expect("the fake must not fail");
        assert_eq!(outcome, RestoreOutcome::Complete(channels));
        assert_eq!(seen.len(), 6, "three chunks on each of two channels");
        assert_eq!(
            seen[0],
            RestoreProgress {
                chunk: 0,
                chunks: 3,
                channel: 0
            }
        );
        assert_eq!(
            seen[5],
            RestoreProgress {
                chunk: 2,
                chunks: 3,
                channel: 1
            }
        );
    }

    #[test]
    fn chunk_starts_cover_the_signal_without_a_redundant_final_chunk() {
        // 18 s at 44.1 kHz with six second chunks and one second of overlap.
        let total = 793_800;
        let starts = chunk_starts(total, 264_600, 44_100);
        assert_eq!(starts, vec![0, 220_500, 441_000, 661_500]);
        assert!(starts[starts.len() - 1] + 264_600 >= total);
        assert!(starts[starts.len() - 2] + 264_600 < total);

        // An input that fits in one chunk starts at zero only.
        assert_eq!(chunk_starts(1_000, 2_000, 100), vec![0]);
    }

    #[test]
    fn crossfade_weights_are_linear_ramps_at_the_edges() {
        let fade = 4;
        let head = crossfade_weights(10, fade, true, false);
        let tail = crossfade_weights(10, fade, false, true);
        assert_eq!(&head[..fade], &[0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0]);
        assert_eq!(&head[fade..], &[1.0; 6]);
        assert_eq!(&tail[..10 - fade], &[1.0; 6]);
        assert_eq!(&tail[10 - fade..], &[1.0, 2.0 / 3.0, 1.0 / 3.0, 0.0]);
        // Adjacent chunks place `ramp[t]` against `ramp[fade - 1 - t]`, and the
        // pair sums to one at every overlap position.
        let ramp = linspace(fade);
        for index in 0..fade {
            assert!((ramp[index] + ramp[fade - 1 - index] - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn chunk_settings_reject_invalid_durations() {
        let rate = f64::from(SAMPLE_RATE);
        let cases: [(Option<f64>, f64, f64, &str); 6] = [
            (Some(0.0), 0.0, 0.0, "positive finite"),
            (Some(f64::INFINITY), 0.0, 0.0, "positive finite"),
            (Some(1.0), -0.1, 0.0, "non-negative finite"),
            (Some(1.0), f64::NAN, 0.0, "non-negative finite"),
            (Some(1.0), 0.0, -1.0, "non-negative finite"),
            (Some(1.0), 0.6, 0.0, "half the chunk"),
        ];
        for (chunk_seconds, overlap_seconds, pad_seconds, message) in cases {
            let error = resolve_chunking(chunk_seconds, overlap_seconds, pad_seconds)
                .expect_err("the settings must fail");
            assert!(
                error.to_string().contains(message),
                "expected {message:?}, got {error}"
            );
        }
        // A chunk shorter than one sample fails after the conversion.
        let error =
            resolve_chunking(Some(0.4 / rate), 0.0, 0.0).expect_err("a sub-sample chunk must fail");
        assert!(error.to_string().contains("shorter than one sample"));
        // No chunking is valid.
        assert_eq!(resolve_chunking(None, 1.0, 1.0).unwrap(), None);
    }

    /// A model that sets a shared cancel flag on its chosen call.
    struct CancellingModel {
        cancel: std::sync::Arc<AtomicBool>,
        at: usize,
        calls: usize,
    }

    impl Model for CancellingModel {
        fn infer(&mut self, samples: &[f32]) -> Result<Vec<f32>, InferenceError> {
            self.calls += 1;
            if self.calls == self.at {
                self.cancel.store(true, Ordering::Relaxed);
            }
            Ok(samples.to_vec())
        }
    }

    /// A model whose every output sample is NaN.
    struct AllNaNModel;

    impl Model for AllNaNModel {
        fn infer(&mut self, samples: &[f32]) -> Result<Vec<f32>, InferenceError> {
            Ok(vec![f32::NAN; samples.len()])
        }
    }

    /// A model that marks NaN from `from` onward on its first call only, so
    /// the NaN lands where the crop discards it.
    struct PadNaNModel {
        from: usize,
        calls: usize,
    }

    impl Model for PadNaNModel {
        fn infer(&mut self, samples: &[f32]) -> Result<Vec<f32>, InferenceError> {
            let mut output = samples.to_vec();
            if self.calls == 0 {
                for sample in &mut output[self.from..] {
                    *sample = f32::NAN;
                }
            }
            self.calls += 1;
            Ok(output)
        }
    }

    #[test]
    fn a_cancel_in_the_last_chunk_returns_cancelled() {
        // 1 s with 0.5 s chunks and 0.1 s overlap gives three chunks. The flag
        // is set inside the last model call, so only a post-call check sees it.
        let samples = ramp(44_100);
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        let backend = ChunkedInference::new(CancellingModel {
            cancel: std::sync::Arc::clone(&cancel),
            at: 3,
            calls: 0,
        });
        let outcome = backend
            .restore(
                std::slice::from_ref(&samples),
                &settings(22_050, 4_410, 0),
                &mut |_| {},
                &cancel,
            )
            .expect("the restore must not fail");
        assert_eq!(outcome, RestoreOutcome::Cancelled);
    }

    #[test]
    fn a_cancel_in_the_full_file_call_returns_cancelled() {
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        let backend = ChunkedInference::new(CancellingModel {
            cancel: std::sync::Arc::clone(&cancel),
            at: 1,
            calls: 0,
        });
        let outcome = backend
            .restore(
                &[vec![0.1; 1_000]],
                &RestoreSettings::default(),
                &mut |_| {},
                &cancel,
            )
            .expect("the restore must not fail");
        assert_eq!(outcome, RestoreOutcome::Cancelled);
    }

    #[test]
    fn a_cancel_in_the_last_channel_returns_cancelled() {
        let channels = vec![vec![0.1; 1_000], vec![0.1; 1_000]];
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        let backend = ChunkedInference::new(CancellingModel {
            cancel: std::sync::Arc::clone(&cancel),
            at: 2,
            calls: 0,
        });
        let outcome = backend
            .restore(&channels, &RestoreSettings::default(), &mut |_| {}, &cancel)
            .expect("the restore must not fail");
        assert_eq!(outcome, RestoreOutcome::Cancelled);
    }

    #[test]
    fn a_short_full_file_output_is_an_error() {
        let error = ChunkedInference::new(ShortOutputModel)
            .restore(
                &[vec![0.1; 1_000]],
                &RestoreSettings::default(),
                &mut |_| {},
                &AtomicBool::new(false),
            )
            .expect_err("a short full-file output must fail");
        assert!(error.to_string().contains("shorter"), "{error}");
    }

    #[test]
    fn a_non_finite_retained_sample_is_an_error() {
        for case in [settings(600, 100, 0), RestoreSettings::default()] {
            let error = ChunkedInference::new(AllNaNModel)
                .restore(
                    &[vec![0.1; 1_000]],
                    &case,
                    &mut |_| {},
                    &AtomicBool::new(false),
                )
                .expect_err("a non-finite sample must fail");
            let message = error.to_string();
            assert!(
                message.contains("channel 0") && message.contains("index"),
                "the error must name the channel and the index: {message}"
            );
        }
    }

    #[test]
    fn a_non_finite_sample_outside_the_crop_is_not_an_error() {
        // Chunk 0 is 600 samples long with 100 samples of pad. Its segment is
        // 800 samples, and the last 200 are discarded. A NaN there does not
        // reach the output.
        let backend = ChunkedInference::new(PadNaNModel {
            from: 600,
            calls: 0,
        });
        let outcome = backend
            .restore(
                &[vec![0.1; 1_000]],
                &settings(600, 100, 100),
                &mut |_| {},
                &AtomicBool::new(false),
            )
            .expect("a discarded NaN must not fail");
        assert!(matches!(outcome, RestoreOutcome::Complete(_)));
    }
}
