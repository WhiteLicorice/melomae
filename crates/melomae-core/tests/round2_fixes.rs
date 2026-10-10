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
