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

use std::path::{Path, PathBuf};

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
    // Trim only the measured transients: the resampler's start-up, and its stop transient, which
    // ends by frame 64 (SNR is 95.2 dB at 48 and 105.8 dB at 64). The old test removed 4000 frames
    // from the end, which cut the top 3.7 kHz of the sweep and left the 20 kHz band unscored.
    let skip = 4_000;
    let end_skip = 64;
    let end = frames - end_skip;
    let snr = snr_db(&reference[skip..end], &ours[skip..end]);
    let top_time = end as f64 / f64::from(TARGET_SAMPLE_RATE);
    let scored_top = 20.0 * (20_000.0f64 / 20.0).powf(top_time / seconds);
    eprintln!("measured: analytic sweep SNR (20 Hz - {scored_top:.0} Hz) = {snr:.1} dB");
    assert!(
        scored_top >= 19_900.0,
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

/// F8: a steady 20 kHz tone is the top of the audible band and is near the 48 kHz input's
/// Nyquist. The corrected resampler must pass it to the 44.1 kHz output against the analytic
/// reference, which puts the gate's coverage at 20 kHz rather than just below it.
#[test]
fn decode_resampled_20khz_tone_matches_the_analytic_reference_above_80_db() {
    let rate = 48_000;
    let frames = rate as usize;
    let input = sine(rate, 20_000.0, 0.5, frames);
    let resampled = AudioBuffer::new(rate, vec![input]).resample_to(TARGET_SAMPLE_RATE);
    let reference = sine(
        TARGET_SAMPLE_RATE,
        20_000.0,
        0.5,
        TARGET_SAMPLE_RATE as usize,
    );

    let ours = resampled.channel(0).unwrap();
    // Trim only the measured transient: the resampler settles within 96 frames at each end, where
    // the tone scores 100.4 dB. A wider margin would hide the transient rather than measure it.
    let end = ours.len().min(reference.len()) - 96;
    let snr = snr_db(&reference[96..end], &ours[96..end]);
    eprintln!("measured: resampled 20 kHz tone SNR = {snr:.1} dB");
    assert!(
        snr >= 80.0,
        "the resampled 20 kHz tone scores {snr:.1} dB, below 80 dB"
    );
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

/// One MPEG-1 Layer III frame at 44.1 kHz is 1152 samples. A headerless MP3 may differ from the
/// oracle by at most that much, which is the estimate's rounding, not lost audio.
const MP3_FRAME: usize = 1_152;

/// Decode `path` natively and read the ffmpeg oracle's count for the same file.
///
/// Returns both counts and prints the measurement, so the task record can carry the numbers
/// before and after the change. The length assertion is separate, so a test that measures two
/// files reports both numbers before either one fails.
fn measure_mp3(fixtures: &Fixtures, path: &Path, label: &str) -> (usize, usize) {
    let decoded = decode_file_native(path).unwrap_or_else(|e| panic!("{label}: {e}"));
    let reference_frames = fixtures.reference_for(path, 44_100, 2, None).len() / 2;
    eprintln!(
        "measured: {label} decoded {} frames, oracle {reference_frames} frames, delta {} samples (one frame = {MP3_FRAME})",
        decoded.frames(),
        decoded.frames().abs_diff(reference_frames)
    );
    assert_eq!(decoded.sample_rate(), 44_100, "{label}: source rate");
    assert_eq!(decoded.channel_count(), 2, "{label}: channel count");
    (decoded.frames(), reference_frames)
}

/// Assert that a headerless MP3 decoded to the oracle's count, within one frame.
fn assert_within_one_mp3_frame(label: &str, decoded: usize, reference: usize) {
    let delta = decoded.abs_diff(reference);
    assert!(
        delta <= MP3_FRAME,
        "{label}: decoded {decoded} frames, oracle {reference} frames, a loss of {delta} samples, \
         more than one MP3 frame ({MP3_FRAME})"
    );
}

/// Task 45: a headerless MP3 states no frame count, so Symphonia estimates one from the first
/// frames' bitrate. The estimate is not a completeness proof, and using it as a trim bound cuts
/// audio off the end. The constant bitrate estimate lands within a frame; it must stay there.
#[test]
fn headerless_cbr_mp3_decodes_every_frame_against_the_oracle() {
    let fixtures = Fixtures::build();
    let source = variable_rate_source(&fixtures);
    let path = fixtures.headerless_mp3("headerless-cbr.mp3", &source, &["-b:a", "128k"]);
    let (frames, reference) = measure_mp3(&fixtures, &path, "headerless CBR MP3");
    assert_within_one_mp3_frame("headerless CBR MP3", frames, reference);
}

/// The variable bitrate case is the one an estimate gets wrong: the first frames need more bits
/// than the average, so the estimated count lands short and the tail is cut.
#[test]
fn headerless_vbr_mp3_decodes_every_frame_against_the_oracle() {
    let fixtures = Fixtures::build();
    let source = variable_rate_source(&fixtures);
    let path = fixtures.headerless_mp3("headerless-vbr.mp3", &source, &["-q:a", "4"]);
    let (frames, reference) = measure_mp3(&fixtures, &path, "headerless VBR MP3");
    assert_within_one_mp3_frame("headerless VBR MP3", frames, reference);
}

/// A terminal APEv2 tag is not audio, but it adds to the physical length that the bitrate
/// estimate divides. It must not change the decoded count either way.
#[test]
fn headerless_vbr_mp3_with_a_terminal_apev2_tag_decodes_every_frame() {
    let fixtures = Fixtures::build();
    let source = variable_rate_source(&fixtures);
    let plain = fixtures.headerless_mp3("headerless-vbr-plain.mp3", &source, &["-q:a", "4"]);
    let tagged = fixtures.headerless_mp3("headerless-vbr-apev2.mp3", &source, &["-q:a", "4"]);
    std::fs::copy(&plain, &tagged).expect("copy the headerless VBR MP3");
    setup::write_apev2_tag(&tagged);

    let (plain_frames, plain_reference) = measure_mp3(&fixtures, &plain, "headerless VBR MP3");
    let (tagged_frames, tagged_reference) =
        measure_mp3(&fixtures, &tagged, "headerless VBR MP3 with an APEv2 tag");
    assert_eq!(
        tagged_frames, plain_frames,
        "the APEv2 tag changed the decoded count from {plain_frames} to {tagged_frames}"
    );
    assert_within_one_mp3_frame("headerless VBR MP3", plain_frames, plain_reference);
    assert_within_one_mp3_frame(
        "headerless VBR MP3 with an APEv2 tag",
        tagged_frames,
        tagged_reference,
    );
}

/// The source for the headerless fixtures: six seconds of a sustained 440 Hz tone.
///
/// The frame layout is what matters, not the music. libmp3lame spends many bits on this source's
/// first frames and about a fifth as many on the rest, so the first sixteen frames average far
/// above the whole file. That is the case the bitrate estimate gets wrong. The project's usual
/// source is stationary: its frames come out near-uniform, the estimate matches the count, and a
/// test built on it stays green with the defect present.
fn variable_rate_source(fixtures: &Fixtures) -> PathBuf {
    let path = fixtures.dir().join("variable-rate-source.wav");
    let frames = 44_100 * 6;
    let tone = sine(44_100, 440.0, 0.5, frames);
    write_wav_f32(&path, 44_100, &[tone.clone(), tone]);
    path
}

/// An MP3 with a Xing, Info, or VBRI header declares its frame count, and that count excludes the
/// encoder delay and padding. The decoder must keep trimming to it, so the padding stays out.
#[test]
fn mp3_with_a_xing_header_keeps_the_gapless_trim() {
    let fixtures = Fixtures::build();
    let decoded = decode_file_native(&fixtures.path("mp3_44100.mp3")).expect("decode the MP3");
    let reference_frames = fixtures.reference("mp3_44100.mp3").len() / 2;
    eprintln!(
        "measured: Xing-header MP3 decoded {} frames, oracle {reference_frames} frames",
        decoded.frames()
    );
    assert_eq!(
        decoded.frames(),
        reference_frames,
        "the Xing-header MP3 no longer matches the oracle's gapless trim"
    );
}
