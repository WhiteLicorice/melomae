//! Decode behavior for every input format, checked against the pinned ffmpeg oracle.
//!
//! Tests that need encoded inputs build them in a temporary directory with the pinned ffmpeg and
//! delete them when the test ends. Tests that only need a synthesized WAV run without ffmpeg.

mod common;
mod setup;

use melomae_core::decode::{
    decode_file, decode_file_native, opus_libopus_version, AudioBuffer, TARGET_SAMPLE_RATE,
};

use common::{
    interleave, log_sweep, log_sweep_seconds, max_abs_diff, rms, sine, snr_db, thd_plus_n_db,
    write_wav_f32,
};
use setup::{Fixtures, TempDir};

/// One generated input, its source rate, and its channel count.
const CASES: &[(&str, u32, usize)] = &[
    ("wav_44100.wav", 44_100, 2),
    ("aiff_44100.aiff", 44_100, 2),
    ("flac_44100.flac", 44_100, 2),
    ("mp3_44100.mp3", 44_100, 2),
    ("aac_44100.m4a", 44_100, 2),
    ("alac_44100.m4a", 44_100, 2),
    ("vorbis_44100.ogg", 44_100, 2),
    ("opus_48000.opus", 48_000, 2),
    ("opus_48000.webm", 48_000, 2),
];

#[test]
fn decode_wav_sine_reaches_the_target_rate_and_keeps_both_channels() {
    let dir = TempDir::new("melomae-wav-stereo");
    let frames = 48_000;
    let left = sine(48_000, 440.0, 0.5, frames);
    let right = sine(48_000, 660.0, 0.5, frames);
    let path = dir.join("sine_48000_stereo.wav");
    write_wav_f32(&path, 48_000, &[left, right]);

    let decoded = decode_file(&path).expect("decode the synthesized WAV");

    assert_eq!(decoded.sample_rate(), TARGET_SAMPLE_RATE);
    assert_eq!(decoded.channel_count(), 2);
    let expected_frames =
        ((frames as f64) * f64::from(TARGET_SAMPLE_RATE) / 48_000.0).ceil() as usize;
    assert_eq!(decoded.frames(), expected_frames);
    assert!(
        rms(decoded.channel(0).unwrap()) > 0.3,
        "left channel lost its tone"
    );
    assert!(
        rms(decoded.channel(1).unwrap()) > 0.3,
        "right channel lost its tone"
    );
}

#[test]
fn decode_wav_sine_native_decode_keeps_the_source_rate_and_length() {
    let dir = TempDir::new("melomae-wav-mono");
    let frames = 24_000;
    let tone = sine(44_100, 1000.0, 0.5, frames);
    let path = dir.join("sine_44100_mono.wav");
    write_wav_f32(&path, 44_100, &[tone]);

    let decoded = decode_file_native(&path).expect("decode the synthesized WAV");

    assert_eq!(decoded.sample_rate(), 44_100);
    assert_eq!(decoded.channel_count(), 1);
    assert_eq!(decoded.frames(), frames);
}

#[test]
fn decode_resampled_tone_distortion_stays_below_minus_100_db() {
    let rate = 48_000;
    let frames = rate as usize * 2;
    let tone = sine(rate, 1000.0, 0.5, frames);
    let buffer = AudioBuffer::new(rate, vec![tone]);

    let resampled = buffer.resample_to(TARGET_SAMPLE_RATE);

    // Measure the steady state: the first and last eighth hold the resampler's start and stop
    // transients, not distortion of the tone itself.
    let channel = resampled.channel(0).unwrap();
    let start = channel.len() / 8;
    let end = channel.len() - channel.len() / 8;
    let thd_plus_n = thd_plus_n_db(&channel[start..end], TARGET_SAMPLE_RATE, 1000.0);
    eprintln!("measured: resampled 1 kHz tone THD+N = {thd_plus_n:.1} dB");
    assert!(
        thd_plus_n <= -100.0,
        "resampled 1 kHz tone THD+N is {thd_plus_n:.1} dB, above the -100 dB target"
    );
}

#[test]
fn decode_synthesized_sweep_resamples_to_the_expected_length() {
    let rate = 48_000;
    let frames = rate as usize * 2;
    let sweep = log_sweep(rate, 20.0, 20_000.0, 0.5, frames);
    let buffer = AudioBuffer::new(rate, vec![sweep]);

    let resampled = buffer.resample_to(TARGET_SAMPLE_RATE);

    let expected =
        ((frames as f64) * f64::from(TARGET_SAMPLE_RATE) / f64::from(rate)).ceil() as usize;
    assert_eq!(resampled.frames(), expected);
    assert!(
        resampled.channel(0).unwrap().iter().all(|s| s.is_finite()),
        "the resampled sweep contains a non-finite sample"
    );
}

#[test]
fn native_decode_length_matches_the_ffmpeg_reference() {
    let fixtures = Fixtures::build();
    let mut mismatches = Vec::new();
    for (name, rate, channels) in CASES {
        let decoded = decode_file_native(&fixtures.path(name)).expect("decode the fixture");
        let reference_frames = fixtures.reference(name).len() / channels;

        if decoded.sample_rate() != *rate {
            mismatches.push(format!(
                "{name}: sample rate {} != {rate}",
                decoded.sample_rate()
            ));
        }
        if decoded.channel_count() != *channels {
            mismatches.push(format!(
                "{name}: {} channels != {channels}",
                decoded.channel_count()
            ));
        }

        // Opus allows one millisecond of slack for pre-skip rounding; every other codec must land
        // within a single sample.
        let tolerance = if name.contains("opus") {
            *rate as usize / 1000
        } else {
            1
        };
        let delta = decoded.frames().abs_diff(reference_frames);
        if delta > tolerance {
            mismatches.push(format!(
                "{name}: decoded {} frames, reference {reference_frames} frames, off by {delta} (tolerance {tolerance})",
                decoded.frames(),
            ));
        }
    }
    assert!(
        mismatches.is_empty(),
        "length mismatches:\n{}",
        mismatches.join("\n")
    );
    eprintln!(
        "checked {} fixtures against the ffmpeg reference",
        CASES.len()
    );
}

#[test]
fn opus_native_decode_matches_the_libopus_reference() {
    let fixtures = Fixtures::build();
    let decoded = decode_file_native(&fixtures.path("opus_48000.opus")).expect("decode opus");
    assert_eq!(decoded.sample_rate(), 48_000, "Opus must decode at 48 kHz");

    let ours = interleave(decoded.channels());
    let reference = fixtures.reference("opus_48000.opus");
    assert_eq!(
        ours.len(),
        reference.len(),
        "decoded and reference sample counts differ"
    );

    // Our libopus and the oracle's libopus are both 1.6-class builds, but they are separate
    // binaries, so they are not bit-exact; see the task file for the measured difference.
    let max = max_abs_diff(&ours, &reference);
    eprintln!(
        "measured: opus max |diff| vs the libopus reference = {max:e} (ours {})",
        opus_libopus_version()
    );
    assert!(
        max <= 1.0e-4,
        "Opus differs from the libopus reference by {max:e}"
    );
}

#[test]
fn decode_resampled_sweep_matches_an_analytic_reference_above_80_db() {
    // The gate: resample a synthesized sweep and score it against the same sweep evaluated at the
    // 44.1 kHz sample times. The phase depends only on time, so the 44.1 kHz sweep is the exact
    // reference for the 48 kHz input.
    let seconds = 3.0;
    let input = log_sweep_seconds(48_000, 20.0, 20_000.0, 0.5, seconds);
    let resampled = AudioBuffer::new(48_000, vec![input]).resample_to(TARGET_SAMPLE_RATE);
    let reference = log_sweep_seconds(TARGET_SAMPLE_RATE, 20.0, 20_000.0, 0.5, seconds);

    let ours = resampled.channel(0).unwrap();
    let frames = ours.len().min(reference.len());
    // Trim only the measured transients: the resampler's start-up, and its short stop transient.
    // The old test removed 4000 frames from the end too, which cut the top 3.7 kHz of the sweep.
    let skip = 4_000;
    let end_skip = 200;
    let end = frames - end_skip;
    let snr = snr_db(&reference[skip..end], &ours[skip..end]);
    let top_time = end as f64 / f64::from(TARGET_SAMPLE_RATE);
    let scored_top = 20.0 * (20_000.0f64 / 20.0).powf(top_time / seconds);
    eprintln!("measured: analytic sweep SNR (20 Hz - {scored_top:.0} Hz) = {snr:.1} dB");
    assert!(
        scored_top >= 19_000.0,
        "the scored sweep only reaches {scored_top:.0} Hz, short of the required 20 kHz band"
    );
    assert!(
        snr >= 80.0,
        "resampled sweep SNR against the analytic reference is {snr:.1} dB, below 80 dB"
    );

    // Recorded, not gated: the same sweep against the ffmpeg swr reference. rubato and swr are
    // different filters with different group delay, so this reads lower than the gate above.
    let fixtures = Fixtures::build();
    let sweep = fixtures.sweep_wav(48_000, seconds, 20.0, 20_000.0);
    let ours = interleave(
        decode_file_native(&sweep)
            .expect("decode the sweep")
            .resample_to(TARGET_SAMPLE_RATE)
            .channels(),
    );
    let reference = fixtures.swr_reference(&sweep, TARGET_SAMPLE_RATE, 2);
    let frames = ours.len().min(reference.len());
    let skip = frames / 16;
    let snr = snr_db(&reference[skip..frames - skip], &ours[skip..frames - skip]);
    eprintln!("measured: sweep SNR vs ffmpeg swr (recorded, not gated) = {snr:.1} dB");
}

#[test]
fn awkward_file_names_decode() {
    let fixtures = Fixtures::build();
    let names = [
        "Florida!!!.opus",
        "Habits (Stay High).opus",
        "Sigur Rós.flac",
    ];
    for name in names {
        let decoded = decode_file(fixtures.path(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            decoded.sample_rate(),
            TARGET_SAMPLE_RATE,
            "{name}: target rate"
        );
        assert!(decoded.frames() > 0, "{name}: decoded no frames");
    }
}
