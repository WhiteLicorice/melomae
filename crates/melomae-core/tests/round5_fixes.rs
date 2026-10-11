//! Task 03 fix round 5: an MP3 whose ID3v2 tag is large must still report a truncated payload.
//!
//! The finding is NEW1: `mp3_declares_frame_count` read a fixed 8 KiB prefix, so a first MPEG frame
//! past that prefix was never seen and the declared-count check was skipped. Each case builds a
//! declared-count MP3 with a larger tag and checks that the intact file decodes to the oracle length
//! and that a cut is an error.

mod common;
mod setup;

use melomae_core::decode::decode_file_native;

use setup::Fixtures;

/// Pad the fixture's ID3v2 tag by `padding` zero bytes, keeping every audio byte, then check the
/// intact file and two cuts. The tag's synchsafe size is updated to match.
fn large_id3_case(fixtures: &Fixtures, label: &str, padding: usize) {
    let bytes = std::fs::read(fixtures.path("mp3_44100.mp3")).expect("read the MP3 fixture");
    assert_eq!(
        &bytes[..3],
        b"ID3",
        "{label}: the fixture must carry an ID3v2 tag"
    );
    let old_size = bytes[6..10]
        .iter()
        .fold(0usize, |value, byte| (value << 7) | usize::from(*byte));
    let size = old_size + padding;
    let mut tagged = bytes[..10 + old_size].to_vec();
    tagged.extend(std::iter::repeat_n(0u8, padding));
    tagged.extend_from_slice(&bytes[10 + old_size..]);
    for index in 0..4 {
        tagged[6 + index] = ((size >> ((3 - index) * 7)) & 127) as u8;
    }

    let intact = fixtures.dir().join(format!("tag-{label}.mp3"));
    std::fs::write(&intact, &tagged).expect("write the tagged MP3");
    let expected = decode_file_native(&intact)
        .unwrap_or_else(|e| panic!("{label}: an intact file must decode: {e}"))
        .frames();
    let oracle = fixtures.reference_for(&intact, 44_100, 2, None);
    assert_eq!(
        expected,
        oracle.len() / 2,
        "{label}: the tag does not change the audio"
    );

    for cut in [17usize, 500] {
        let truncated = fixtures.dir().join(format!("tag-{label}-cut-{cut}.mp3"));
        std::fs::write(&truncated, &tagged[..tagged.len() - cut]).expect("write the cut MP3");
        match decode_file_native(&truncated) {
            Ok(buffer) => panic!(
                "{label}: a {cut}-byte cut decoded {} of {expected} frames",
                buffer.frames()
            ),
            Err(error) => eprintln!("{label} cut={cut}: {:?}: {}", error.kind(), error.reason()),
        }
    }
}

/// NEW1: a declared frame count behind a tag larger than the old 8 KiB prefix must still be found,
/// so a cut is an error. The three cases place the first MPEG frame below, across, and above the old
/// 8 KiB boundary.
#[test]
fn declared_mp3_large_id3_still_rejects_cuts() {
    let fixtures = Fixtures::build();
    let bytes = std::fs::read(fixtures.path("mp3_44100.mp3")).expect("read the MP3 fixture");
    let old_size = bytes[6..10]
        .iter()
        .fold(0usize, |value, byte| (value << 7) | usize::from(*byte));
    let across = 8_192usize.saturating_sub(10 + old_size);
    for (label, padding) in [("below", 1_000usize), ("across", across), ("above", 16_384)] {
        large_id3_case(&fixtures, label, padding);
    }
}
