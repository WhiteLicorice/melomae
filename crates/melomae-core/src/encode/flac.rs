//! FLAC writing: 24-bit, at the input sample rate, with the pure-Rust `flacenc` encoder.
//!
//! `flacenc` supports up to 24 bits per sample (`flacenc::constant::MAX_BITS_PER_SAMPLE`), so the
//! pure-Rust encoder covers the required depth and no C library is needed. Samples are rounded to
//! the nearest 24-bit value with no dither. Samples outside the +/-1.0 range are clamped to the
//! 24-bit rails, and the count of clamped samples is returned.

use std::io::Write;

use flacenc::component::BitRepr;
use flacenc::error::Verify;

use crate::decode::AudioBuffer;

use super::{EncodeFailure, EncodeOutcome, WriteResult};

/// The FLAC sample depth. The task requires 24 bits, and `flacenc` supports exactly that maximum.
const BITS_PER_SAMPLE: usize = 24;

/// The full scale of the signed 24-bit range: 2^23.
const SCALE: f64 = 8_388_608.0;

/// The largest 24-bit sample, `2^23 - 1`.
const MAX_SAMPLE: i32 = 8_388_607;

/// The smallest 24-bit sample, `-2^23`.
const MIN_SAMPLE: i32 = -8_388_608;

pub(super) fn write(writer: &mut dyn Write, source: &AudioBuffer) -> WriteResult {
    let channels = source.channel_count();
    let frames = source.frames();

    let mut samples = Vec::with_capacity(frames * channels);
    let mut clamped = 0usize;
    for frame in 0..frames {
        for channel in source.channels() {
            let sample = channel[frame];
            if !(-1.0..=1.0).contains(&sample) {
                clamped += 1;
            }
            samples.push(quantize(sample));
        }
    }

    let config = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|(_, error)| {
            EncodeFailure::Message(format!("the FLAC encoder config is invalid: {error}"))
        })?;
    let block_size = config.block_size;
    let block = flacenc::source::MemSource::from_samples(
        &samples,
        channels,
        BITS_PER_SAMPLE,
        source.sample_rate() as usize,
    );
    let mut stream = flacenc::encode_with_fixed_block_size(&config, block, block_size)
        .map_err(|error| EncodeFailure::Message(format!("the FLAC encoder failed: {error}")))?;
    // flacenc lowers the STREAMINFO minimum block size to the shortest frame, which is the last,
    // partial frame. A stream that uses the fixed blocking strategy must declare the same minimum
    // and maximum block size, or a decoder reads it as variable-block and rejects the frame
    // numbers. Restore the fixed-size declaration before writing.
    stream
        .stream_info_mut()
        .set_block_sizes(block_size, block_size)
        .map_err(|error| EncodeFailure::Message(format!("set the FLAC block sizes: {error}")))?;
    let mut sink = flacenc::bitsink::ByteSink::new();
    stream.write(&mut sink).map_err(|error| {
        EncodeFailure::Message(format!("the FLAC stream is malformed: {error}"))
    })?;
    writer.write_all(sink.as_slice())?;
    Ok(EncodeOutcome { clamped })
}

/// Quantize one float sample to the signed 24-bit range, rounding to nearest and clamping to the
/// rails. `flacenc` requires input samples in `-2^23 ..= 2^23 - 1`.
fn quantize(sample: f32) -> i32 {
    let scaled = (f64::from(sample) * SCALE).round();
    scaled.clamp(f64::from(MIN_SAMPLE), f64::from(MAX_SAMPLE)) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantize_rounds_to_nearest_and_clamps_to_the_rails() {
        assert_eq!(quantize(0.0), 0);
        assert_eq!(quantize(0.5), 4_194_304);
        assert_eq!(quantize(-0.5), -4_194_304);
        assert_eq!(quantize(1.0), MAX_SAMPLE, "the positive rail is 2^23 - 1");
        assert_eq!(quantize(-1.0), MIN_SAMPLE);
        assert_eq!(quantize(2.0), MAX_SAMPLE);
        assert_eq!(quantize(-9.0), MIN_SAMPLE);
    }

    #[test]
    fn a_flac_stream_records_24_bit_samples() {
        let source = AudioBuffer::new(44_100, vec![vec![0.25; 16]]);
        let mut bytes = Vec::new();
        write(&mut bytes, &source).expect("write the FLAC");

        assert_eq!(&bytes[0..4], b"fLaC");
        let block_header = &bytes[4..8];
        assert_eq!(
            block_header[0] & 0x7F,
            0,
            "the first block must be STREAMINFO"
        );
        let length = u32::from_be_bytes([0, block_header[1], block_header[2], block_header[3]]);
        assert_eq!(length, 34, "STREAMINFO is 34 bytes");

        let info = &bytes[8..8 + 34];
        let sample_rate = (usize::from(info[10]) << 12)
            | (usize::from(info[11]) << 4)
            | (usize::from(info[12]) >> 4);
        let channels = ((info[12] >> 1) & 0x07) + 1;
        let bits_per_sample =
            ((usize::from(info[12] & 0x01) << 4) | (usize::from(info[13]) >> 4)) + 1;
        assert_eq!(sample_rate, 44_100);
        assert_eq!(channels, 1);
        assert_eq!(
            bits_per_sample, 24,
            "the stream must declare 24-bit samples"
        );
    }

    #[test]
    fn a_flac_reports_each_sample_outside_the_unit_range() {
        let source = AudioBuffer::new(44_100, vec![vec![1.5, -1.5, 0.5, 1.0, -1.0]]);
        let mut bytes = Vec::new();
        let outcome = write(&mut bytes, &source).expect("write the FLAC");
        assert_eq!(outcome.clamped, 2);
    }
}
