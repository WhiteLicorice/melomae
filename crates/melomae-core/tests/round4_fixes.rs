//! Task 03 fix round 4: the MP3 declared-count and family-255 WebM cases, through the public API.
//!
//! Each test builds its input in a temporary directory and removes it when the test ends. E1 is the
//! finding: an MP3 with no frame-count header must still decode when a terminal tag is appended.

mod common;
mod setup;

use melomae_core::decode::{decode_file_native, DecodeErrorKind};

use setup::Fixtures;

/// E1: an MP3 with no Xing, Info, or VBRI header has no declared frame count, so the decoder must
/// not compare its decoded total against Symphonia's bitrate estimate. Appending a terminal APEv2
/// tag shifts the physical length, which shifts that estimate, and an intact file must still decode.
#[test]
fn headerless_mp3_with_terminal_tag_is_intact() {
    let fixtures = Fixtures::build();
    let mut bytes = std::fs::read(fixtures.path("mp3_44100.mp3")).expect("read the MP3 fixture");
    let at = bytes
        .windows(4)
        .position(|window| window == b"Info" || window == b"Xing")
        .expect("the fixture carries an Info frame");
    let frame_start = at - 36; // MPEG-1 stereo side information follows the four-byte header.
    let header = u32::from_be_bytes(bytes[frame_start..frame_start + 4].try_into().unwrap());
    let frame_len = 144 * 320_000 / 44_100 + ((header >> 9) & 1) as usize;
    bytes.drain(frame_start..frame_start + frame_len);
    let bare = fixtures.dir().join("no-count.mp3");
    std::fs::write(&bare, &bytes).expect("write the headerless MP3");
    let baseline = decode_file_native(&bare).expect("decode the headerless MP3");

    let text = vec![b'x'; 8192];
    let mut item = Vec::new();
    item.extend((text.len() as u32).to_le_bytes());
    item.extend(0u32.to_le_bytes());
    item.extend(b"Comment\0".iter().copied());
    item.extend(text);
    let mut footer = Vec::new();
    footer.extend(b"APETAGEX");
    footer.extend(2000u32.to_le_bytes());
    footer.extend(((item.len() + 32) as u32).to_le_bytes());
    footer.extend(1u32.to_le_bytes());
    footer.extend(0u32.to_le_bytes());
    footer.extend([0u8; 8]);
    bytes.extend(item);
    bytes.extend(footer);
    let tagged = fixtures.dir().join("no-count-ape.mp3");
    std::fs::write(&tagged, bytes).expect("write the tagged MP3");

    let bare_oracle = fixtures.reference_for(&bare, 44_100, 2, None);
    let oracle = fixtures.reference_for(&tagged, 44_100, 2, None);
    assert_eq!(
        oracle, bare_oracle,
        "the terminal tag does not change the audio"
    );

    let decoded = decode_file_native(&tagged).expect("an intact headerless MP3 must decode");
    assert_eq!(decoded.frames(), baseline.frames());
}

/// D4: the same family-255 scope applies in a WebM container. Remuxing the committed family-1
/// fixture without re-encoding, then setting only the `OpusHead` family byte, must return the named
/// `UnsupportedFormat`.
#[test]
fn family_255_webm_names_its_scope() {
    let fixtures = Fixtures::build();
    let source = fixtures.multichannel_opus();
    let webm = fixtures.dir().join("family1.webm");
    let output = std::process::Command::new(setup::ffmpeg())
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(&source)
        .args(["-c:a", "copy"])
        .arg(&webm)
        .output()
        .expect("remux the family-1 fixture");
    assert!(output.status.success(), "remux the existing fixture");
    assert_eq!(decode_file_native(&webm).unwrap().channel_count(), 6);

    let mut bytes = std::fs::read(&webm).expect("read the remuxed WebM");
    let at = bytes
        .windows(8)
        .position(|window| window == b"OpusHead")
        .expect("the WebM carries an OpusHead");
    assert_eq!(bytes[at + 18], 1);
    bytes[at + 18] = 255;
    let bad = fixtures.dir().join("family255.webm");
    std::fs::write(&bad, bytes).expect("write the family-255 WebM");

    let error = decode_file_native(&bad).expect_err("family 255 is out of scope");
    assert_eq!(error.kind(), DecodeErrorKind::UnsupportedFormat);
    let reason = error.reason().to_string();
    eprintln!("webm family 255: {reason}");
    assert!(reason.contains("mapping family 255"), "{reason}");
    assert!(reason.contains("6 channels"), "{reason}");
}
