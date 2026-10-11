//! End-to-end restore through the public entry a later pipeline task consumes:
//! `melomae_core::inference::RestoreBackend`, fed by the task 03 decoder.
//!
//! Each test builds its input in a temporary directory and removes it when the
//! test ends.

mod common;
mod setup;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use melomae_core::decode::decode_file;
use melomae_core::inference::{
    ChunkedInference, InferenceError, Model, RestoreBackend, RestoreOutcome, RestoreProgress,
    RestoreSettings, SAMPLE_RATE,
};

use common::{sine, write_wav_f32};
use setup::TempDir;

/// An identity model, so the restored channels must equal the input.
struct IdentityModel;

impl Model for IdentityModel {
    fn infer(&mut self, samples: &[f32]) -> Result<Vec<f32>, InferenceError> {
        Ok(samples.to_vec())
    }
}

/// An identity model that counts its calls.
struct CountingModel {
    calls: Arc<AtomicUsize>,
}

impl Model for CountingModel {
    fn infer(&mut self, samples: &[f32]) -> Result<Vec<f32>, InferenceError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(samples.to_vec())
    }
}

/// Write a two channel sine WAV, decode it, and return its planar channels.
fn decoded_stereo(dir: &TempDir) -> Vec<Vec<f32>> {
    let path = dir.join("stereo.wav");
    let left = sine(SAMPLE_RATE, 440.0, 0.4, SAMPLE_RATE as usize);
    let right = sine(SAMPLE_RATE, 660.0, 0.4, SAMPLE_RATE as usize);
    write_wav_f32(&path, SAMPLE_RATE, &[left, right]);
    let audio = decode_file(&path).expect("decode the stereo fixture");
    assert_eq!(audio.sample_rate(), SAMPLE_RATE);
    audio.channels().to_vec()
}

#[test]
fn e2e_inference_restore_backend_restores_a_decoded_file_channel_by_channel() {
    let dir = TempDir::new("melomae-inference-e2e");
    let channels = decoded_stereo(&dir);
    assert_eq!(channels.len(), 2);

    let backend = ChunkedInference::new(IdentityModel);
    let settings = RestoreSettings {
        chunk_seconds: Some(0.5),
        overlap_seconds: 0.1,
        chunk_pad_seconds: 0.05,
    };
    let mut seen = Vec::new();
    let outcome = backend
        .restore(
            &channels,
            &settings,
            &mut |progress| seen.push(progress),
            &AtomicBool::new(false),
        )
        .expect("the restore must succeed");

    match outcome {
        RestoreOutcome::Complete(restored) => {
            assert_eq!(restored.len(), channels.len());
            for (got, want) in restored.iter().zip(&channels) {
                assert!(
                    common::max_abs_diff(got, want) <= 1e-6,
                    "the identity model must reproduce the input"
                );
            }
        }
        RestoreOutcome::Cancelled => panic!("the run must not cancel"),
    }

    // Every channel reports chunk zero of a chunk count greater than one.
    for channel in 0..channels.len() {
        let first: Vec<&RestoreProgress> = seen
            .iter()
            .filter(|progress| progress.channel == channel)
            .collect();
        assert!(!first.is_empty(), "channel {channel} must report progress");
        assert_eq!(first[0].chunk, 0);
        assert!(first[0].chunks > 1);
    }
}

#[test]
fn e2e_inference_cancel_returns_no_channels() {
    let dir = TempDir::new("melomae-inference-cancel");
    let channels = decoded_stereo(&dir);

    let calls = Arc::new(AtomicUsize::new(0));
    let backend = ChunkedInference::new(CountingModel {
        calls: Arc::clone(&calls),
    });
    let settings = RestoreSettings {
        chunk_seconds: Some(0.25),
        overlap_seconds: 0.05,
        chunk_pad_seconds: 0.0,
    };
    let cancel = AtomicBool::new(false);
    let outcome = backend
        .restore(
            &channels,
            &settings,
            &mut |progress| {
                if progress.chunk == 1 {
                    cancel.store(true, Ordering::Relaxed);
                }
            },
            &cancel,
        )
        .expect("the restore must not fail");

    assert_eq!(outcome, RestoreOutcome::Cancelled);
    assert_eq!(
        calls.load(Ordering::Relaxed),
        2,
        "the run stops within one chunk of the cancel"
    );
}
