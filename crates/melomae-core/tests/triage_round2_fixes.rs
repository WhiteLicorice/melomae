//! Fix round 2 for task 16: the start selection must reach the end of a short file.
//!
//! Every input is built in test setup and removed in teardown. Nothing is committed.

mod common;
mod setup;

use melomae_core::decode::decode_file;
use melomae_core::store::Verdict;
use melomae_core::triage::triage_file;

use common::write_wav_f32;
use setup::{calibration_source_path, TempDir};

/// The committed calibration source, decoded to planar f32 at 44.1 kHz.
fn source() -> Vec<Vec<f32>> {
    decode_file(calibration_source_path())
        .expect("decode the calibration source")
        .channels()
        .to_vec()
}

#[test]
fn triage_short_file_trailing_music_is_measured() {
    let dir = TempDir::new("melomae-fix16-short");
    let src = source();
    let tail: Vec<Vec<f32>> = src.iter().map(|channel| channel[..3808].to_vec()).collect();

    // The control holds the audio alone, so its only start covers it.
    let control = dir.join("control.wav");
    write_wav_f32(&control, 44_100, &tail);
    let control_row = triage_file(&control).expect("triage the control");
    assert_eq!(
        control_row.verdict,
        Verdict::Lossless,
        "control: {control_row:?}"
    );

    // The short file opens with 8192 silent frames. Its only valid start, `span`, is past that
    // silence, so the audio must still set the cutoff.
    let planes: Vec<Vec<f32>> = tail
        .iter()
        .map(|channel| {
            let mut plane = vec![0.0f32; 8192];
            plane.extend_from_slice(channel);
            plane
        })
        .collect();
    let path = dir.join("short.wav");
    write_wav_f32(&path, 44_100, &planes);
    let row = triage_file(&path).expect("triage the short file");
    assert_eq!(row.verdict, Verdict::Lossless, "short: {row:?}");
    assert!(
        row.spectral_cutoff_hz.unwrap_or(0.0) >= 21_000.0,
        "short: {:?}",
        row.spectral_cutoff_hz
    );
}

#[test]
fn triage_short_adaptive_window_file_with_trailing_audio_is_measured() {
    let dir = TempDir::new("melomae-fix16-adaptive");
    let src = source();

    // 6000 frames. The window falls to 4096, and the span (1904) is below the hop (2048), so a
    // one-position selection covers only silence. The audio starts at frame 4096, past that window.
    let planes: Vec<Vec<f32>> = src
        .iter()
        .map(|channel| {
            let mut plane = vec![0.0f32; 4096];
            plane.extend_from_slice(&channel[..1904]);
            plane
        })
        .collect();
    let path = dir.join("adaptive.wav");
    write_wav_f32(&path, 44_100, &planes);
    let row = triage_file(&path).expect("triage the adaptive file");
    assert_eq!(row.verdict, Verdict::Lossless, "{row:?}");
    assert!(
        row.spectral_cutoff_hz.unwrap_or(0.0) >= 21_000.0,
        "{:?}",
        row.spectral_cutoff_hz
    );
}
