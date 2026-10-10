//! Task 03 fix round 2: the blocking container findings, exercised through the public API.
//!
//! Every test builds its input in a temporary directory and removes it when the test ends. Each
//! one corresponds to a finding in `review-03c.md`: B1 (a truncated non-Opus file is an error),
//! B4 (the `OpusHead` output gain), and C2 (the documented channel order).

mod common;
mod setup;

use melomae_core::decode::decode_file_native;

use setup::Fixtures;

/// B1: a WAV cut inside its data chunk must return an error, not decode a short stream. The
/// decoder previously accepted every `UnexpectedEof` from the demuxer as a clean end of file, so
/// a truncated WAV silently lost 1000 frames.
#[test]
fn b1_truncated_wav_data_is_a_typed_error() {
    let fixtures = Fixtures::build();
    let source = fixtures.path("wav_44100.wav");
    let bytes = std::fs::read(&source).expect("read the WAV fixture");
    let truncated = fixtures.dir().join("wav-cut.wav");
    std::fs::write(&truncated, &bytes[..bytes.len() - 8_000]).expect("write the truncated WAV");

    let result = std::panic::catch_unwind(|| decode_file_native(&truncated));
    assert!(result.is_ok(), "a truncated WAV must not panic");
    match result.unwrap() {
        Ok(buffer) => panic!(
            "a truncated WAV decoded {} frames instead of returning an error",
            buffer.frames()
        ),
        Err(error) => eprintln!(
            "truncated WAV: kind={:?} reason={}",
            error.kind(),
            error.reason()
        ),
    }
}

/// B4: the `OpusHead` output gain (signed Q7.8 dB at bytes 16 and 17) must scale the decoded
/// audio. The decoder previously ignored it, so a gained file differed from the libopus oracle by
/// about the gain itself. The gain applies once, in both the single-stream and the multistream
/// path.
#[test]
fn b4_opus_output_gain_matches_the_libopus_oracle() {
    let fixtures = Fixtures::build();
    // The single-stream path is mapping family 0 (stereo); the multistream path is family 1 (six
    // channels). Both sources stay below full scale after +6 dB, so the oracle does not clip. The
    // multichannel mapping is the codec's Vorbis order, which differs from the oracle's order.
    let cases: [(&str, _, usize, &[usize]); 2] = [
        ("stereo", fixtures.quiet_stereo_opus(), 2, &[0, 1]),
        (
            "multichannel",
            fixtures.multichannel_opus(),
            6,
            &[0, 2, 1, 4, 5, 3],
        ),
    ];
    for gain in [1_536i16, -1_536i16] {
        for (name, source, channels, order) in &cases {
            let target = fixtures.dir().join(format!("gain-{name}-{gain}.opus"));
            setup::rewrite_opus_head(source, &target, Some(gain), None);

            let decoded = decode_file_native(&target).expect("decode the gained Opus");
            assert_eq!(decoded.channel_count(), *channels, "{name}: channel count");

            let reference = common::planar_from_interleaved(
                &fixtures.reference_for(&target, 48_000, *channels, Some("libopus")),
                *channels,
            );
            assert_eq!(decoded.frames(), reference[0].len(), "{name}: frames");

            let max = decoded
                .channels()
                .iter()
                .enumerate()
                .map(|(index, ours)| common::max_abs_diff(ours, &reference[order[index]]))
                .fold(0.0f32, f32::max);
            eprintln!("gain {gain} on {name}: max |diff| = {max:e}");
            assert!(
                max <= 1.0e-4,
                "gain {gain} on {name} differs from the oracle by {max:e}"
            );
        }
    }
}
