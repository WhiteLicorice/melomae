//! Decode behavior for every input format, checked against the ffmpeg oracle.
//!
//! Tests that need a generated fixture skip with a message when
//! `tools/fixtures/make_fixtures.py` has not produced it. The threshold and distortion tests
//! synthesize their own input and always run.

mod common;

use std::path::PathBuf;

use melomae_core::decode::{
    decode_file, decode_file_native, opus_libopus_version, AudioBuffer, TARGET_SAMPLE_RATE,
};

use common::{
    fixture, interleave, log_sweep, log_sweep_seconds, max_abs_diff, read_f32le, rms, sine, snr_db,
    thd_plus_n_db, write_wav_f32,
};

/// One generated input and its ffmpeg reference decode.
struct Case {
    /// The input fixture file name.
    input: &'static str,
    /// The sample rate the input holds, and the rate its reference was decoded at.
    rate: u32,
    /// The channel count.
    channels: usize,
    /// The raw little-endian f32 reference decode written by the generator.
    reference: &'static str,
}

const CASES: &[Case] = &[
    Case {
        input: "wav_44100.wav",
        rate: 44_100,
        channels: 2,
        reference: "wav_44100.wav.ffmpeg.f32",
    },
    Case {
        input: "aiff_44100.aiff",
        rate: 44_100,
        channels: 2,
        reference: "aiff_44100.aiff.ffmpeg.f32",
    },
    Case {
        input: "flac_44100.flac",
        rate: 44_100,
        channels: 2,
        reference: "flac_44100.flac.ffmpeg.f32",
    },
    Case {
        input: "mp3_44100.mp3",
        rate: 44_100,
        channels: 2,
        reference: "mp3_44100.mp3.ffmpeg.f32",
    },
    Case {
        input: "aac_44100.m4a",
        rate: 44_100,
        channels: 2,
        reference: "aac_44100.m4a.ffmpeg.f32",
    },
    Case {
        input: "alac_44100.m4a",
        rate: 44_100,
        channels: 2,
        reference: "alac_44100.m4a.ffmpeg.f32",
    },
    Case {
        input: "vorbis_44100.ogg",
        rate: 44_100,
        channels: 2,
        reference: "vorbis_44100.ogg.ffmpeg.f32",
    },
    Case {
        input: "opus_48000.opus",
        rate: 48_000,
        channels: 2,
        reference: "opus_48000.opus.ffmpeg.f32",
    },
    Case {
        input: "opus_48000.webm",
        rate: 48_000,
        channels: 2,
        reference: "opus_48000.webm.ffmpeg.f32",
    },
];

/// A short WAV written into the process temp directory for the always-on tests.
fn temp_wav(name: &str, sample_rate: u32, planes: &[Vec<f32>]) -> PathBuf {
    let path = std::env::temp_dir().join("melomae-decode-tests").join(name);
    write_wav_f32(&path, sample_rate, planes);
    path
}

#[test]
fn decode_wav_sine_reaches_the_target_rate_and_keeps_both_channels() {
    let frames = 48_000;
    let left = sine(48_000, 440.0, 0.5, frames);
    let right = sine(48_000, 660.0, 0.5, frames);
    let path = temp_wav("sine_48000_stereo.wav", 48_000, &[left, right]);

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
fn wav_sine_native_decode_keeps_the_source_rate_and_length() {
    let frames = 24_000;
    let tone = sine(44_100, 1000.0, 0.5, frames);
    let path = temp_wav("sine_44100_mono.wav", 44_100, &[tone]);

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
    let mut checked = 0;
    let mut mismatches = Vec::new();
    for case in CASES {
        let (Some(input), Some(reference)) = (fixture(case.input), fixture(case.reference)) else {
            continue;
        };
        let decoded = decode_file_native(&input).expect("decode the fixture");
        let reference_samples = read_f32le(&reference);
        let reference_frames = reference_samples.len() / case.channels;

        if decoded.sample_rate() != case.rate {
            mismatches.push(format!(
                "{}: sample rate {} != {}",
                case.input,
                decoded.sample_rate(),
                case.rate
            ));
        }
        if decoded.channel_count() != case.channels {
            mismatches.push(format!(
                "{}: {} channels != {}",
                case.input,
                decoded.channel_count(),
                case.channels
            ));
        }

        // Opus allows one millisecond of slack for pre-skip rounding; every other codec must
        // land within a single sample.
        let tolerance = if case.input.contains("opus") {
            case.rate as usize / 1000
        } else {
            1
        };
        let delta = decoded.frames().abs_diff(reference_frames);
        if delta > tolerance {
            mismatches.push(format!(
                "{}: decoded {} frames, reference {} frames, off by {} (tolerance {})",
                case.input,
                decoded.frames(),
                reference_frames,
                delta,
                tolerance,
            ));
        }
        checked += 1;
    }
    if checked == 0 {
        eprintln!("skipped: no generated fixtures; run tools/fixtures/make_fixtures.py");
    } else {
        eprintln!("checked {checked} fixtures against the ffmpeg reference");
    }
    assert!(
        mismatches.is_empty(),
        "length mismatches:\n{}",
        mismatches.join("\n")
    );
}

#[test]
fn opus_native_decode_matches_the_libopus_reference() {
    let Some(input) = fixture("opus_48000.opus") else {
        eprintln!("skipped: opus fixture absent; run tools/fixtures/make_fixtures.py");
        return;
    };
    let Some(reference) = fixture("opus_48000.opus.ffmpeg.f32") else {
        eprintln!("skipped: opus reference absent; run tools/fixtures/make_fixtures.py");
        return;
    };

    let decoded = decode_file_native(&input).expect("decode the opus fixture");
    assert_eq!(decoded.sample_rate(), 48_000, "Opus must decode at 48 kHz");

    let ours = interleave(decoded.channels());
    let reference = read_f32le(&reference);
    assert_eq!(
        ours.len(),
        reference.len(),
        "decoded and reference sample counts differ"
    );

    // The shipped libopus and the oracle's libopus are both 1.6.1, but they are separate builds,
    // so their SIMD paths round differently. Versions: ours is the libopus 1.6.1 bundled by
    // opusic-sys 0.7.5; the oracle is conda-forge libopus 1.6.1 via `ffmpeg -c:a libopus`. The
    // measured max |diff| is 1.53e-5 against a 1e-4 tolerance.
    let max = max_abs_diff(&ours, &reference);
    eprintln!(
        "measured: opus max |diff| vs the libopus reference = {max:e} (ours libopus {}, oracle conda-forge libopus 1.6.1)",
        opus_libopus_version()
    );
    assert!(
        max <= 1.0e-4,
        "Opus differs from the libopus reference by {max:e} (ours libopus {}, oracle conda-forge libopus 1.6.1), above the 1e-4 tolerance",
        opus_libopus_version()
    );
}

#[test]
fn decode_resampled_sweep_matches_an_analytic_reference_above_80_db() {
    // The gate: resample a synthesized sweep and score it against the same sweep evaluated at the
    // 44.1 kHz sample times. The phase depends only on time, so the 44.1 kHz sweep is the exact
    // reference for the 48 kHz input in the band up to 20 kHz.
    let seconds = 3.0;
    let input = log_sweep_seconds(48_000, 20.0, 20_000.0, 0.5, seconds);
    let resampled = AudioBuffer::new(48_000, vec![input]).resample_to(TARGET_SAMPLE_RATE);
    let reference = log_sweep_seconds(TARGET_SAMPLE_RATE, 20.0, 20_000.0, 0.5, seconds);

    let ours = resampled.channel(0).unwrap();
    let frames = ours.len().min(reference.len());
    let skip = 4_000;
    let snr = snr_db(&reference[skip..frames - skip], &ours[skip..frames - skip]);
    eprintln!("measured: analytic sweep SNR (20 Hz - 20 kHz) = {snr:.1} dB");
    assert!(
        snr >= 80.0,
        "resampled sweep SNR against the analytic reference is {snr:.1} dB, below 80 dB"
    );

    // Recorded, not gated: the same sweep against the ffmpeg swr reference. rubato and swr are
    // different filters with different group delay, so this reads lower than the gate above.
    if let (Some(input), Some(reference)) =
        (fixture("sweep_48000.wav"), fixture("sweep_44100.swr.f32"))
    {
        let native = decode_file_native(&input).expect("decode the sweep fixture");
        let ours = interleave(native.resample_to(TARGET_SAMPLE_RATE).channels());
        let reference = read_f32le(&reference);
        let frames = ours.len().min(reference.len());
        let skip = frames / 16;
        let snr = snr_db(&reference[skip..frames - skip], &ours[skip..frames - skip]);
        eprintln!("measured: sweep SNR vs ffmpeg swr (recorded, not gated) = {snr:.1} dB");
    } else {
        eprintln!("skipped: sweep swr fixtures absent; run tools/fixtures/make_fixtures.py");
    }
}

#[test]
fn awkward_file_names_decode() {
    let names = [
        "Florida!!!.opus",
        "Habits (Stay High).opus",
        "Sigur Rós.flac",
    ];
    let mut checked = 0;
    for name in names {
        let Some(path) = fixture(name) else {
            continue;
        };
        let decoded = decode_file(&path).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(
            decoded.sample_rate(),
            TARGET_SAMPLE_RATE,
            "{name}: target rate"
        );
        assert!(decoded.frames() > 0, "{name}: decoded no frames");
        checked += 1;
    }
    if checked == 0 {
        eprintln!("skipped: no named fixtures; run tools/fixtures/make_fixtures.py");
    }
}
