//! Task 03 fix round 3: the remaining container findings, exercised through the public API.
//!
//! Every test builds its input in a temporary directory and removes it when the test ends. Each
//! one corresponds to a finding in `review-03d.md`.

mod common;
mod setup;

use melomae_core::decode::{decode_file_native, DecodeErrorKind};

use setup::Fixtures;

/// D3: a present but malformed MP4 edit list must return an error, not silently fall back to an
/// untrimmed decode. The old reader ended its box iteration at the bad size, `find_box` reported
/// absence, and the decoder decoded the whole media without the edit's start trim.
#[test]
fn malformed_edit_list_does_not_fall_back() {
    let fixtures = Fixtures::build();
    let mut bytes = std::fs::read(fixtures.path("aac_44100.m4a")).expect("read the AAC fixture");
    let at = bytes
        .windows(4)
        .position(|window| window == b"elst")
        .expect("the fixture carries an edit list");
    let old = u32::from_be_bytes(bytes[at - 4..at].try_into().unwrap());
    bytes[at - 4..at].copy_from_slice(&(old + 1).to_be_bytes());
    let bad = fixtures.dir().join("malformed-elst.m4a");
    std::fs::write(&bad, bytes).expect("write the malformed MP4");

    match decode_file_native(&bad) {
        Ok(buffer) => panic!(
            "a malformed edit list decoded {} frames instead of returning an error",
            buffer.frames()
        ),
        Err(error) => {
            eprintln!(
                "malformed edit list: kind={:?} reason={}",
                error.kind(),
                error.reason()
            );
            assert_eq!(error.kind(), DecodeErrorKind::Malformed);
        }
    }
}

/// D4: a mapping-family 255 Ogg Opus file is out of scope for this task. It must return
/// `UnsupportedFormat` whose reason names the family and the channel count, not the probe's
/// rejection of the header.
#[test]
fn family_255_opus_names_itself_unsupported() {
    let fixtures = Fixtures::build();
    let source = fixtures.multichannel_opus();
    let file = fixtures.dir().join("family255.opus");
    setup::rewrite_opus_head(&source, &file, None, Some(255));

    let error = decode_file_native(&file).expect_err("family 255 is out of scope");
    assert_eq!(error.kind(), DecodeErrorKind::UnsupportedFormat);
    let reason = error.reason().to_string();
    eprintln!("family 255: {reason}");
    assert!(reason.contains("mapping family 255"), "{reason}");
    assert!(reason.contains("6 channels"), "{reason}");
}
