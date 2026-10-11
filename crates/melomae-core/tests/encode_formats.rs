//! Encode behavior for every output format, checked against the task 03 decoder and the pinned
//! ffmpeg oracle.
//!
//! Tests that need encoded inputs build them in a temporary directory and delete them when the
//! test ends. The Opus and ffmpeg tests use the pinned ffmpeg only as an oracle, never to encode.

mod common;
mod setup;

use std::path::Path;
use std::process::Command;

use melomae_core::decode::{decode_file, AudioBuffer, TARGET_SAMPLE_RATE};
use melomae_core::encode::{encode, EncodeOutcome, OutputFormat};

use common::{log_sweep, read_f32le, sine};
use setup::TempDir;

/// The 24-bit quantization the FLAC encoder must apply: round to nearest with a 2^23 scale, then
/// clamp to the signed 24-bit range. This is an independent implementation, written from the FLAC
/// sample representation rather than from the encoder's code.
fn quantize_24(sample: f32) -> f32 {
    let scaled = (f64::from(sample) * 8_388_608.0).round();
    let clamped = scaled.clamp(-8_388_608.0, 8_388_607.0);
    (clamped as i32 as f32) / 8_388_608.0
}

/// The largest normalized cross-correlation of `test` against `reference`, over every integer lag
/// within `max_lag` samples. The two signals need not be the same length.
fn best_correlation(reference: &[f32], test: &[f32], max_lag: isize) -> f64 {
    let mut best = f64::NEG_INFINITY;
    for lag in -max_lag..=max_lag {
        let mut cross = 0.0f64;
        let mut reference_energy = 0.0f64;
        let mut test_energy = 0.0f64;
        for (index, reference_sample) in reference.iter().enumerate() {
            let shifted = index as isize + lag;
            if shifted < 0 || shifted as usize >= test.len() {
                continue;
            }
            let r = f64::from(*reference_sample);
            let t = f64::from(test[shifted as usize]);
            cross += r * t;
            reference_energy += r * r;
            test_energy += t * t;
        }
        if reference_energy == 0.0 || test_energy == 0.0 {
            continue;
        }
        let correlation = cross / (reference_energy.sqrt() * test_energy.sqrt());
        best = best.max(correlation);
    }
    best
}

/// Two distinct 44.1 kHz stereo tones, so a wrong channel order or a dropped channel is visible.
fn stereo_tone(rate: u32, frames: usize) -> Vec<Vec<f32>> {
    vec![
        sine(rate, 440.0, 0.6, frames),
        sine(rate, 660.0, 0.6, frames),
    ]
}

/// Decode `path` with the pinned ffmpeg to raw f32 and return the sample count.
fn ffmpeg_decodes(path: &Path, scratch: &TempDir) -> usize {
    let target = scratch.join("ffmpeg.f32");
    let output = Command::new(setup::ffmpeg())
        .args(["-hide_banner", "-loglevel", "error", "-y"])
        .arg("-i")
        .arg(path)
        .args(["-f", "f32le", "-c:a", "pcm_f32le"])
        .arg(&target)
        .output()
        .expect("run the pinned ffmpeg");
    assert!(
        output.status.success(),
        "the pinned ffmpeg failed to open {}: {}\n{}",
        path.display(),
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    read_f32le(&target).len()
}

/// Test-first plan item 1: a FLAC round trip through the task 03 decoder equals the 24-bit
/// quantized input exactly.
#[test]
fn e2e_encode_flac_round_trip_equals_the_24_bit_quantized_input() {
    let dir = TempDir::new("melomae-encode-flac");
    let rate = 44_100;
    let frames = 44_100;
    let planes = stereo_tone(rate, frames);
    let source = AudioBuffer::new(rate, planes.clone());
    let target = dir.join("restored.flac");

    let outcome = encode(&source, &target, OutputFormat::Flac).expect("encode FLAC");

    assert_eq!(outcome.clamped, 0, "a bounded tone must not clamp");
    let decoded = decode_file(&target).expect("decode the FLAC we wrote");
    assert_eq!(decoded.sample_rate(), TARGET_SAMPLE_RATE);
    assert_eq!(decoded.channel_count(), 2);
    assert_eq!(decoded.frames(), frames, "the FLAC length must be exact");
    for (channel, plane) in planes.iter().enumerate() {
        let decoded_channel = decoded.channel(channel).unwrap();
        for (frame, sample) in plane.iter().enumerate() {
            let expected = quantize_24(*sample);
            let actual = decoded_channel[frame];
            assert_eq!(
                actual, expected,
                "channel {channel} frame {frame}: decoded {actual} != quantized {expected}"
            );
        }
    }
}

/// Test-first plan item 2: a WAV round trip is bit-exact, including samples outside +/-1.0, which
/// the float format must carry unchanged and must not count as clamps.
#[test]
fn e2e_encode_wav_round_trip_is_bit_exact() {
    let dir = TempDir::new("melomae-encode-wav");
    let rate = 44_100;
    let frames = 4_096;
    let mut left = sine(rate, 300.0, 0.5, frames);
    let right = sine(rate, 700.0, 1.4, frames);
    // Two samples outside the integer range, to prove the float path keeps them and clamps nothing.
    left[10] = 1.75;
    left[11] = -2.5;
    let planes = vec![left, right];
    let source = AudioBuffer::new(rate, planes.clone());
    let target = dir.join("restored.wav");

    let outcome = encode(&source, &target, OutputFormat::Wav).expect("encode WAV");

    assert_eq!(outcome.clamped, 0, "the WAV float format clamps nothing");
    let decoded = decode_file(&target).expect("decode the WAV we wrote");
    assert_eq!(decoded.sample_rate(), TARGET_SAMPLE_RATE);
    assert_eq!(decoded.channel_count(), 2);
    assert_eq!(decoded.frames(), frames);
    for (channel, plane) in planes.iter().enumerate() {
        let decoded_channel = decoded.channel(channel).unwrap();
        for (frame, sample) in plane.iter().enumerate() {
            assert_eq!(
                decoded_channel[frame].to_bits(),
                sample.to_bits(),
                "channel {channel} frame {frame} is not bit-exact"
            );
        }
    }
}

/// Test-first plan item 3: an Opus output decodes, its length matches within 1 ms, and its aligned
/// correlation with the input is at least 0.95.
#[test]
fn e2e_encode_opus_decodes_and_correlates_with_the_input() {
    let dir = TempDir::new("melomae-encode-opus");
    let rate = 44_100;
    let frames = 44_100;
    let sweep = log_sweep(rate, 60.0, 15_000.0, 0.6, frames);
    let planes = vec![sweep.clone(), sine(rate, 440.0, 0.6, frames)];
    let source = AudioBuffer::new(rate, planes.clone());
    let target = dir.join("restored.opus");

    let outcome = encode(&source, &target, OutputFormat::Opus).expect("encode Opus");

    assert_eq!(outcome.clamped, 0, "the Opus path clamps nothing");
    let decoded = decode_file(&target).expect("decode the Opus we wrote");
    assert_eq!(decoded.sample_rate(), TARGET_SAMPLE_RATE);

    // 1 ms at the target rate.
    let tolerance = TARGET_SAMPLE_RATE as usize / 1_000;
    let delta = decoded.frames().abs_diff(frames);
    assert!(
        delta <= tolerance,
        "Opus length {} is off the input {frames} by {delta}, more than {tolerance} frames",
        decoded.frames()
    );

    let correlation = best_correlation(&planes[0], decoded.channel(0).unwrap(), 200);
    assert!(
        correlation >= 0.95,
        "the decoded Opus correlates {correlation:.4} with the input, below 0.95"
    );
}

/// Test-first plan item 4: an input with samples above 1.0 reports the correct clamp count, for an
/// integer format.
#[test]
fn e2e_encode_reports_the_clamp_count_from_an_overflowing_input() {
    let dir = TempDir::new("melomae-encode-clamp");
    let samples = vec![1.5f32, -1.5, 0.5, 1.0, -1.0, 0.0, 3.0];
    let source = AudioBuffer::new(44_100, vec![samples.clone()]);
    let target = dir.join("restored.flac");

    let outcome: EncodeOutcome = encode(&source, &target, OutputFormat::Flac).expect("encode FLAC");

    assert_eq!(outcome.clamped, 3, "1.5, -1.5, and 3.0 are outside +/-1.0");
    let decoded = decode_file(&target).expect("decode the FLAC we wrote");
    let channel = decoded.channel(0).unwrap();
    assert_eq!(
        channel[0],
        8_388_607.0 / 8_388_608.0,
        "1.5 must clamp to the rail"
    );
    assert_eq!(channel[1], -1.0, "-1.5 must clamp to the rail");
    assert_eq!(channel[2], quantize_24(0.5));
    assert_eq!(
        channel[6],
        8_388_607.0 / 8_388_608.0,
        "3.0 must clamp to the rail"
    );
}

/// Acceptance: every output opens in the pinned ffmpeg and decodes to samples.
#[test]
fn e2e_encode_every_output_decodes_in_the_pinned_ffmpeg() {
    let dir = TempDir::new("melomae-encode-ffmpeg");
    let rate = 44_100;
    let frames = 44_100;
    let source = AudioBuffer::new(rate, stereo_tone(rate, frames));

    for (format, name) in [
        (OutputFormat::Flac, "restored.flac"),
        (OutputFormat::Wav, "restored.wav"),
        (OutputFormat::Opus, "restored.opus"),
    ] {
        let target = dir.join(name);
        encode(&source, &target, format).unwrap_or_else(|e| panic!("encode {name}: {e}"));
        let samples = ffmpeg_decodes(&target, &dir);
        assert!(
            samples > 0,
            "the pinned ffmpeg decoded no samples from {name}"
        );
    }
}

/// Scope item 5: the encoder writes a temporary file and renames it only when the file is complete.
/// A finished encode leaves the target and no leftover temporary file.
#[test]
fn e2e_encode_leaves_only_the_target_behind() {
    let dir = TempDir::new("melomae-encode-atomic");
    let source = AudioBuffer::new(44_100, stereo_tone(44_100, 4_096));
    let target = dir.join("restored.flac");

    encode(&source, &target, OutputFormat::Flac).expect("encode FLAC");

    let entries: Vec<String> = std::fs::read_dir(dir.path())
        .expect("read the output directory")
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(entries, vec!["restored.flac".to_string()]);
}

/// The sorted file names in a directory.
fn directory_entries(path: &Path) -> Vec<String> {
    let mut entries: Vec<String> = std::fs::read_dir(path)
        .expect("read the directory")
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    entries.sort();
    entries
}

/// A normal stereo tone with one chosen sample value at the given channel and frame.
fn tone_with(channel: usize, frame: usize, value: f32) -> AudioBuffer {
    let mut planes = stereo_tone(44_100, 4_410);
    planes[channel][frame] = value;
    AudioBuffer::new(44_100, planes)
}

/// Fix round 1, B1: a NaN sample must be rejected with a named error, for every format.
#[test]
fn e2e_encode_rejects_a_nan_sample_in_every_format() {
    let dir = TempDir::new("melomae-encode-nan");
    for (format, name) in [
        (OutputFormat::Flac, "nan.flac"),
        (OutputFormat::Wav, "nan.wav"),
        (OutputFormat::Opus, "nan.opus"),
    ] {
        let target = dir.join(name);
        let error = encode(&tone_with(1, 10, f32::NAN), &target, format)
            .expect_err("a NaN sample must be rejected");
        let message = error.to_string();
        assert!(message.contains("channel 1"), "{name}: {message}");
        assert!(message.contains("frame 10"), "{name}: {message}");
        assert!(
            !target.exists(),
            "{name}: a rejected encode must not create the target"
        );
    }
    assert!(
        directory_entries(dir.path()).is_empty(),
        "no file may remain"
    );
}

/// Fix round 1, B1: either infinity sign must be rejected. FLAC carries the positive sign and Opus
/// the negative sign.
#[test]
fn e2e_encode_rejects_infinities() {
    let dir = TempDir::new("melomae-encode-infinity");

    let error = encode(
        &tone_with(1, 5, f32::INFINITY),
        &dir.join("infinity.flac"),
        OutputFormat::Flac,
    )
    .expect_err("positive infinity must be rejected");
    assert!(error.to_string().contains("frame 5"), "{error}");

    let error = encode(
        &tone_with(0, 7, f32::NEG_INFINITY),
        &dir.join("negative.opus"),
        OutputFormat::Opus,
    )
    .expect_err("negative infinity must be rejected");
    assert!(error.to_string().contains("frame 7"), "{error}");

    assert!(
        directory_entries(dir.path()).is_empty(),
        "no file may remain"
    );
}

/// Fix round 1, B1: a rejected encode must not touch an older target and must leave no temporary
/// file.
#[test]
fn e2e_encode_rejects_without_touching_an_older_target() {
    let dir = TempDir::new("melomae-encode-older-target");
    let target = dir.join("restored.flac");
    let older = b"an older restored file".to_vec();
    std::fs::write(&target, &older).expect("write the older target");

    encode(&tone_with(0, 3, f32::NAN), &target, OutputFormat::Flac)
        .expect_err("a NaN sample must be rejected");

    assert_eq!(
        std::fs::read(&target).expect("read the older target"),
        older,
        "the older target must stay byte-identical"
    );
    assert_eq!(
        directory_entries(dir.path()),
        vec!["restored.flac".to_string()],
        "no temporary file may remain"
    );
}

/// Fix round 1, B1: a rejected encode must not create the output folder.
#[test]
fn e2e_encode_creates_no_folder_for_a_rejected_input() {
    let dir = TempDir::new("melomae-encode-no-folder");
    let missing = dir.path().join("missing");
    let target = missing.join("restored.wav");

    encode(&tone_with(0, 1, f32::NAN), &target, OutputFormat::Wav)
        .expect_err("a NaN sample must be rejected");

    assert!(!missing.exists(), "a rejected encode must create no folder");
}

/// Fix round 1, N1: a sample rate other than 44100 Hz must be rejected before any file is written.
#[test]
fn e2e_encode_rejects_an_unsupported_sample_rate() {
    let dir = TempDir::new("melomae-encode-rate");
    for rate in [0u32, 48_000] {
        for (format, name) in [
            (OutputFormat::Flac, "rate.flac"),
            (OutputFormat::Wav, "rate.wav"),
            (OutputFormat::Opus, "rate.opus"),
        ] {
            let target = dir.join(&format!("{rate}-{name}"));
            let source = AudioBuffer::new(rate, stereo_tone(44_100, 1_024));
            let error = encode(&source, &target, format)
                .expect_err("a rate other than 44100 must be rejected");
            assert!(error.to_string().contains("44100"), "rate {rate}: {error}");
            assert!(!target.exists(), "rate {rate}: no target may remain");
        }
    }
    assert!(
        directory_entries(dir.path()).is_empty(),
        "no file may remain"
    );
}

/// Fix round 1, N2: an empty buffer must be rejected for every format.
#[test]
fn e2e_encode_rejects_an_empty_buffer() {
    let dir = TempDir::new("melomae-encode-empty");
    for (format, name) in [
        (OutputFormat::Flac, "empty.flac"),
        (OutputFormat::Wav, "empty.wav"),
        (OutputFormat::Opus, "empty.opus"),
    ] {
        let target = dir.join(name);
        let source = AudioBuffer::new(44_100, vec![Vec::new(), Vec::new()]);
        let error = encode(&source, &target, format).expect_err("an empty buffer must be rejected");
        assert!(error.to_string().contains("no frames"), "{name}: {error}");
        assert!(
            !target.exists(),
            "{name}: a rejected encode must not create the target"
        );
    }
    assert!(
        directory_entries(dir.path()).is_empty(),
        "no file may remain"
    );
}
