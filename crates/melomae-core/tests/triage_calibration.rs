//! The task 16 calibration: measured cutoffs set the triage thresholds.
//!
//! Every fixture is built in test setup with the pinned ffmpeg, from a synthesized full-band source,
//! and removed in teardown. The test prints the measured table and then checks each fixture against
//! its expected verdict from SRS §4.3.

mod common;
mod setup;

use melomae_core::decode::decode_file;
use melomae_core::store::Verdict;
use melomae_core::triage::{measure_cutoff_hz, measure_cutoff_hz_at, triage_file};

use setup::{Calibration, CALIBRATION_SOURCE};

/// The expected verdict of each calibration fixture (SRS §4.3).
///
/// The source WAV passes through lossless, so it is `lossless`. The 320 kb/s MP3 is the pitch
/// example for `low-benefit`. The FLAC transcoded from the 128 kb/s MP3 carries that lossy cutoff
/// under a lossless container, so it is `suspect-lossless`.
const EXPECTED: &[(&str, Verdict)] = &[
    (CALIBRATION_SOURCE, Verdict::Lossless),
    ("cal_mp3_64.mp3", Verdict::Restore),
    ("cal_mp3_128.mp3", Verdict::Restore),
    ("cal_mp3_192.mp3", Verdict::Restore),
    ("cal_mp3_320.mp3", Verdict::LowBenefit),
    ("cal_aac_128.m4a", Verdict::Restore),
    ("cal_aac_256.m4a", Verdict::LowBenefit),
    ("cal_flac_from_mp3_128.flac", Verdict::SuspectLossless),
    ("cal_flac_lossless.flac", Verdict::Lossless),
];

#[test]
fn triage_calibration_fixtures_get_the_documented_verdicts() {
    let calibration = Calibration::build();

    eprintln!("calibration table (measured cutoff in Hz):");
    for (name, _) in EXPECTED {
        let path = calibration.path(name);
        let item = triage_file(&path).expect("triage a calibration fixture");
        let audio = decode_file(&path).expect("decode a calibration fixture");
        let sweep: Vec<String> = [40.0, 60.0, 80.0, 100.0, 120.0, 140.0]
            .iter()
            .map(|drop| format!("{drop:.0}dB={:.0}", measure_cutoff_hz_at(&audio, *drop)))
            .collect();
        eprintln!(
            "  {name}: verdict={} codec={:?} cutoff={:.0} bitrate_kbps={:?} bit_depth={:?}  {}",
            item.verdict.as_str(),
            item.codec,
            item.spectral_cutoff_hz.unwrap_or(0.0),
            item.bitrate_kbps,
            item.bit_depth,
            sweep.join("  "),
        );
    }

    let source = decode_file(&calibration.path(CALIBRATION_SOURCE)).expect("decode the source");
    let source_cutoff = measure_cutoff_hz(&source);
    assert!(
        source_cutoff >= 21_000.0,
        "the calibration source must reach 21 kHz, measured {source_cutoff}"
    );
    eprintln!("source cutoff: {source_cutoff:.0} Hz");

    for (name, expected) in EXPECTED {
        let item = triage_file(&calibration.path(name)).expect("triage a calibration fixture");
        assert_eq!(item.verdict, *expected, "{name}");
    }
}

#[test]
fn triage_reports_a_truncated_file_as_unreadable_with_a_reason() {
    let calibration = Calibration::build();
    let source = calibration.path("cal_mp3_128.mp3");
    let bytes = std::fs::read(&source).expect("read the source MP3");
    let truncated = calibration.path("cal_truncated.mp3");
    std::fs::write(&truncated, &bytes[..bytes.len() / 2]).expect("write the truncated MP3");

    let item = triage_file(&truncated).expect("triage the truncated MP3");

    assert_eq!(item.verdict, Verdict::Unreadable);
    assert!(
        item.reason
            .as_deref()
            .is_some_and(|reason| !reason.is_empty()),
        "an unreadable file must carry a reason, got {:?}",
        item.reason
    );
    assert!(
        item.spectral_cutoff_hz.is_none(),
        "an unreadable file has no cutoff"
    );
}
