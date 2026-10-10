//! Task 03 fix round 1: the blocking container findings, exercised through the public API.
//!
//! Every test builds its input in a temporary directory and removes it when the test ends. Each
//! one corresponds to a finding in `review-03.md`: F2 (whole-file readers), F1 (hostile padding),
//! F3 (the selected track), F4 (signed interior padding), and F5 (a clean EOF).

mod common;
mod setup;

use std::path::Path;

use melomae_core::decode::{decode_file, decode_file_native};

use setup::{Fixtures, Padding};

/// F2: no decode reader may load the whole file. This is the source-scan counterpart of the
/// reader's `fs::read` use, as `decode_source_never_spawns_a_process` is for ffmpeg.
#[test]
fn f2_readers_do_not_load_the_whole_file() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("decode");
    for name in ["matroska.rs", "mp4.rs"] {
        let path = dir.join(name);
        let text = std::fs::read_to_string(&path).expect("read a decode reader source file");
        for needle in ["fs::read(", "fs::read_to_string("] {
            assert!(
                !text.contains(needle),
                "{} loads the whole file ({needle})",
                path.display()
            );
        }
    }
}

/// F1: `DiscardPadding` of `i64::MAX` nanoseconds, on every block, must return a typed error
/// instead of overflowing the reader and panicking through `decode_file`.
#[test]
fn f1_hostile_padding_returns_a_typed_error() {
    let fixtures = Fixtures::build();
    let hostile = fixtures.dir().join("overflow-padding.webm");
    setup::rewrite_webm_padding(
        &fixtures.path("opus_48000.webm"),
        &hostile,
        Padding::Hostile,
    );

    let result = std::panic::catch_unwind(|| decode_file(&hostile));
    assert!(result.is_ok(), "the decoder panicked on hostile padding");
    let error = result
        .unwrap()
        .expect_err("hostile padding must return a typed error");
    assert!(
        error.reason().contains("DiscardPadding"),
        "the hostile padding must be reported as such, got: {}",
        error.reason()
    );
}

/// F3: a two-track WebM's selected track (track 1) must be trimmed with its own `CodecDelay` and
/// `DiscardPadding`, so it matches the ffmpeg oracle for `-map 0:a:0`.
#[test]
fn f3_selected_track_matches_its_own_oracle() {
    let fixtures = Fixtures::build();
    let source = fixtures.path("opus_48000.webm");
    // Both orders: the selected (first) track ordinary with the low-delay track after it, and the
    // reverse. The decoder must always use the selected track's own trim.
    for low_delay in [1u32, 0u32] {
        let two_tracks = fixtures.two_track_webm(&source, low_delay);
        let decoded = decode_file_native(&two_tracks).expect("decode the two-track WebM");
        let reference = fixtures.reference_for(&two_tracks, 48_000, 2, Some("libopus"));

        let ours = common::interleave(decoded.channels());
        assert_eq!(
            ours.len(),
            reference.len(),
            "decoded and reference lengths (low-delay index {low_delay})"
        );
        let max = common::max_abs_diff(&ours, &reference);
        assert!(
            max <= 1.0e-4,
            "the selected track differs from its oracle by {max} (low-delay index {low_delay})"
        );
    }
}

/// F4: a positive interior `DiscardPadding` trims its own block's end, a negative one trims that
/// block's start. At 48 kHz, 1 ms is 48 samples, so both decode to `unpadded - 48`, and the
/// `CodecDelay` pre-skip is applied once, not twice.
#[test]
fn f4_signed_interior_padding_trims_exactly_its_block() {
    let fixtures = Fixtures::build();
    let source = fixtures.path("opus_48000.webm");
    let baseline = decode_file_native(&source)
        .expect("decode the baseline")
        .frames();

    let positive = fixtures.dir().join("interior-positive.webm");
    let negative = fixtures.dir().join("interior-negative.webm");
    setup::rewrite_webm_padding(&source, &positive, Padding::Interior(1_000_000));
    setup::rewrite_webm_padding(&source, &negative, Padding::Interior(-1_000_000));

    let positive_frames = decode_file_native(&positive)
        .expect("positive padding")
        .frames();
    let negative_frames = decode_file_native(&negative)
        .expect("negative padding")
        .frames();
    assert_eq!(
        positive_frames,
        baseline - 48,
        "positive padding must trim 48 samples from the block end"
    );
    assert_eq!(
        negative_frames,
        baseline - 48,
        "negative padding must trim 48 samples from the block start"
    );
}

/// F5: a WebM written with the live muxer option ends with an unknown-size Segment. That clean end
/// of file must decode, not raise an unexpected-end-of-file error, and it must keep its exact
/// length.
#[test]
fn f5_live_webm_decodes_at_clean_eof() {
    let fixtures = Fixtures::build();
    let live = fixtures.live_webm(&fixtures.path("opus_48000.webm"));
    let decoded = decode_file_native(&live).expect("decode the live WebM at a clean end of file");
    assert_eq!(
        decoded.frames(),
        288_000,
        "the live WebM holds six seconds at 48 kHz"
    );
}

/// F6: an initial empty MP4 edit (`media_time = -1`) carries no media. The decoder must skip it
/// and apply the following edit's start trim, as ffmpeg does.
#[test]
fn f6_an_initial_empty_edit_keeps_the_following_start_trim() {
    let fixtures = Fixtures::build();
    let empty_edit = fixtures.dir().join("empty-edit.m4a");
    setup::prepend_empty_edit(&fixtures.path("aac_44100.m4a"), &empty_edit);

    let decoded = decode_file_native(&empty_edit).expect("decode the empty-edit MP4");
    let reference = fixtures.reference_for(&empty_edit, 44_100, 2, None);
    assert_eq!(
        decoded.frames(),
        reference.len() / 2,
        "the decoder must apply the trim of the first non-empty edit"
    );
}

/// F5: a WebM cut in the middle of a cluster is a truncation, not a clean end of file. It must
/// return an error, not decode a partial stream.
#[test]
fn f5_a_truncated_webm_is_not_a_clean_eof() {
    let fixtures = Fixtures::build();
    let bytes = std::fs::read(fixtures.path("opus_48000.webm")).expect("read the source WebM");
    let truncated = fixtures.dir().join("truncated.webm");
    std::fs::write(&truncated, &bytes[..bytes.len() - 500]).expect("write the truncated WebM");

    let result = std::panic::catch_unwind(|| decode_file(&truncated));
    assert!(result.is_ok(), "a truncated file must not panic");
    assert!(
        result.unwrap().is_err(),
        "a WebM cut inside a cluster must return an error"
    );
}

/// F7: a valid six-channel Opus file must keep all six channels. Each source channel carries a
/// distinct tone, and every oracle channel must match exactly one decoded channel, so no channel
/// was dropped, duplicated, or down-mixed. The decoder emits channels in the `OpusHead` mapping
/// order, which need not equal ffmpeg's output order, so the match is a permutation.
#[test]
fn f7_multichannel_opus_keeps_every_channel() {
    let fixtures = Fixtures::build();
    let source = fixtures.multichannel_opus();
    let decoded = decode_file_native(&source).expect("decode the six-channel Opus");

    assert_eq!(decoded.sample_rate(), 48_000, "Opus decodes at 48 kHz");
    assert_eq!(
        decoded.channel_count(),
        6,
        "the decoder must keep six channels"
    );

    let reference = fixtures.reference_for(&source, 48_000, 6, Some("libopus"));
    let reference = common::planar_from_interleaved(&reference, 6);
    assert_eq!(
        decoded.frames(),
        reference[0].len(),
        "decoded frames vs the libopus oracle"
    );

    let mut matched = [false; 6];
    for (oracle_channel, expected) in reference.iter().enumerate() {
        let (index, diff) = decoded
            .channels()
            .iter()
            .enumerate()
            .filter(|(index, _)| !matched[*index])
            .map(|(index, plane)| (index, common::max_abs_diff(plane, expected)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .expect("a free decoded channel");
        assert!(
            diff <= 1.0e-4,
            "oracle channel {oracle_channel} has no matching decoded channel (best {index}, diff {diff})"
        );
        matched[index] = true;
    }
}
