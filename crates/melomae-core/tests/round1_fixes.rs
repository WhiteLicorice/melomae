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
    setup::rewrite_webm_padding(&fixtures.path("opus_48000.webm"), &hostile, Padding::Hostile);

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
    let two_tracks = fixtures.two_track_webm(&fixtures.path("opus_48000.webm"));
    let decoded = decode_file_native(&two_tracks).expect("decode the two-track WebM");
    let reference = fixtures.reference_for(&two_tracks, 48_000, 2, Some("libopus"));

    let ours = common::interleave(decoded.channels());
    assert_eq!(ours.len(), reference.len(), "decoded and reference lengths");
    let max = common::max_abs_diff(&ours, &reference);
    assert!(
        max <= 1.0e-4,
        "the selected track differs from its oracle by {max}"
    );
}

/// F4: a negative interior `DiscardPadding` discards from that block's start. The old reader added
/// it to the absolute end total, so a negative value made the decode *longer*. It must never
/// lengthen the audio.
#[test]
fn f4_negative_interior_padding_does_not_lengthen_the_audio() {
    let fixtures = Fixtures::build();
    let source = fixtures.path("opus_48000.webm");
    let positive = fixtures.dir().join("interior-positive.webm");
    let negative = fixtures.dir().join("interior-negative.webm");
    setup::rewrite_webm_padding(&source, &positive, Padding::Interior(1_000_000));
    setup::rewrite_webm_padding(&source, &negative, Padding::Interior(-1_000_000));

    let positive_frames = decode_file_native(&positive).expect("positive padding").frames();
    let negative_frames = decode_file_native(&negative).expect("negative padding").frames();
    assert!(
        negative_frames <= positive_frames,
        "negative interior padding lengthened the audio: {negative_frames} > {positive_frames}"
    );
}

/// F5: a WebM written with the live muxer option ends with an unknown-size Segment. That clean end
/// of file must decode, not raise an unexpected-end-of-file error.
#[test]
fn f5_live_webm_decodes_at_clean_eof() {
    let fixtures = Fixtures::build();
    let live = fixtures.live_webm(&fixtures.path("opus_48000.webm"));
    let decoded = decode_file_native(&live).expect("decode the live WebM at a clean end of file");
    assert!(decoded.frames() > 0, "the live WebM decoded no frames");
}
