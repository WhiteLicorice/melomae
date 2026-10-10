//! End-to-end decode through the public entry a later engine task consumes:
//! `melomae_core::decode::decode_file`, which returns planar 32-bit float at 44.1 kHz.

mod common;
mod setup;

use melomae_core::decode::{decode_file, decode_file_native, TARGET_SAMPLE_RATE};

use common::{log_sweep, sine, write_wav_f32};
use setup::{Fixtures, TempDir, INPUTS};

/// The 44.1 kHz frame count that resampling `frames` at `rate` must produce.
fn expected_target_frames(frames: usize, rate: u32) -> usize {
    ((frames as f64) * f64::from(TARGET_SAMPLE_RATE) / f64::from(rate)).ceil() as usize
}

#[test]
fn e2e_decode_synthesized_wav_to_44100_float() {
    let dir = TempDir::new("melomae-e2e-wav");
    let frames = 48_000;
    let left = sine(48_000, 440.0, 0.5, frames);
    let right = sine(48_000, 880.0, 0.5, frames);
    let path = dir.join("e2e_sine_48000.wav");
    write_wav_f32(&path, 48_000, &[left, right]);

    let decoded = decode_file(&path).expect("decode the synthesized WAV end to end");

    assert_eq!(decoded.sample_rate(), TARGET_SAMPLE_RATE);
    assert_eq!(decoded.channel_count(), 2);
    assert_eq!(decoded.frames(), expected_target_frames(frames, 48_000));
}

#[test]
fn e2e_decode_every_fixture_format_to_44100_float() {
    let fixtures = Fixtures::build();
    for (name, _, _) in INPUTS {
        let path = fixtures.path(name);
        let native = decode_file_native(&path).expect("decode natively");
        let decoded = decode_file(&path).expect("decode to 44.1 kHz");

        assert_eq!(
            decoded.sample_rate(),
            TARGET_SAMPLE_RATE,
            "{name}: target rate"
        );
        assert_eq!(
            decoded.channel_count(),
            native.channel_count(),
            "{name}: channel count"
        );
        assert_eq!(
            decoded.frames(),
            expected_target_frames(native.frames(), native.sample_rate()),
            "{name}: resampled length",
        );
    }
    eprintln!("decoded {} fixtures end to end to 44.1 kHz", INPUTS.len());
}

#[test]
fn e2e_decode_reports_a_named_error_for_an_undecodable_input() {
    let dir = TempDir::new("melomae-e2e-bad");
    let path = dir.join("not_audio.bin");
    std::fs::write(&path, b"this is not audio at all, not even close").unwrap();

    let error = decode_file(&path).expect_err("a text file must not decode");
    assert!(
        error.to_string().contains("not_audio.bin"),
        "the error must name the file: {error}"
    );
    assert!(!error.reason().is_empty(), "the error must carry a reason");
}

#[test]
fn e2e_decode_resampled_sweep_is_finite_and_ordered() {
    let dir = TempDir::new("melomae-e2e-sweep");
    let frames = 24_000;
    let sweep = log_sweep(48_000, 20.0, 20_000.0, 0.5, frames);
    let path = dir.join("e2e_sweep_48000.wav");
    write_wav_f32(&path, 48_000, &[sweep]);

    let decoded = decode_file(&path).expect("decode the synthesized sweep end to end");

    assert_eq!(decoded.frames(), expected_target_frames(frames, 48_000));
    let channel = decoded.channel(0).unwrap();
    assert!(channel.iter().all(|s| s.is_finite()));
    // A 20 Hz..20 kHz sweep is a full-scale signal throughout; its second half is not silent.
    let tail = &channel[channel.len() / 2..];
    let rms = (tail
        .iter()
        .map(|s| f64::from(*s) * f64::from(*s))
        .sum::<f64>()
        / tail.len() as f64)
        .sqrt();
    assert!(
        rms > 0.1,
        "the resampled sweep tail is unexpectedly quiet (rms {rms})"
    );
}
