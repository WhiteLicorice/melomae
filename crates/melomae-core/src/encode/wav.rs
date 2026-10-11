//! WAV writing: IEEE float 32-bit, at the input sample rate.
//!
//! The file is a canonical RIFF/WAVE with a 16-byte `fmt ` chunk and one `data` chunk. The format
//! tag is `WAVE_FORMAT_IEEE_FLOAT` (3), so samples keep their exact float value, including values
//! outside the +/-1.0 range that a normalized integer format cannot hold.

use std::io::Write;

use crate::decode::AudioBuffer;

use super::{EncodeFailure, EncodeOutcome, WriteResult};

/// The number of frames interleaved per write, to bound the temporary buffer.
const BLOCK_FRAMES: usize = 4096;

/// The `fmt ` chunk format tag for IEEE float samples.
const WAVE_FORMAT_IEEE_FLOAT: u16 = 0x0003;

pub(super) fn write(writer: &mut dyn Write, source: &AudioBuffer) -> WriteResult {
    let channels = source.channel_count();
    let frames = source.frames();
    let rate = source.sample_rate();

    let block_align = u16::try_from(channels * 4).map_err(|_| {
        EncodeFailure::Message(format!("{channels} channels do not fit the WAV header"))
    })?;
    let byte_rate = rate
        .checked_mul(u32::from(block_align))
        .ok_or_else(|| EncodeFailure::Message("the WAV byte rate overflows".to_string()))?;
    let data_len = u32::try_from(frames * channels * 4).map_err(|_| {
        EncodeFailure::Message("the audio is larger than the RIFF format allows".to_string())
    })?;
    let riff_len = 36u32.checked_add(data_len).ok_or_else(|| {
        EncodeFailure::Message("the file is larger than the RIFF format allows".to_string())
    })?;

    let header = WavHeader {
        riff_len,
        format: WAVE_FORMAT_IEEE_FLOAT,
        channels: channels as u16,
        rate,
        byte_rate,
        block_align,
        bits_per_sample: 32,
        data_len,
    };
    header.write(writer)?;

    let mut block = Vec::with_capacity(BLOCK_FRAMES * channels * 4);
    let mut frame = 0;
    while frame < frames {
        let end = (frame + BLOCK_FRAMES).min(frames);
        block.clear();
        for index in frame..end {
            for channel in source.channels() {
                block.extend_from_slice(&channel[index].to_le_bytes());
            }
        }
        writer.write_all(&block)?;
        frame = end;
    }
    Ok(EncodeOutcome { clamped: 0 })
}

/// The bytes of the RIFF header, up to and including the `data` chunk length.
struct WavHeader {
    riff_len: u32,
    format: u16,
    channels: u16,
    rate: u32,
    byte_rate: u32,
    block_align: u16,
    bits_per_sample: u16,
    data_len: u32,
}

impl WavHeader {
    fn write(&self, writer: &mut dyn Write) -> std::io::Result<()> {
        writer.write_all(b"RIFF")?;
        writer.write_all(&self.riff_len.to_le_bytes())?;
        writer.write_all(b"WAVE")?;
        writer.write_all(b"fmt ")?;
        writer.write_all(&16u32.to_le_bytes())?;
        writer.write_all(&self.format.to_le_bytes())?;
        writer.write_all(&self.channels.to_le_bytes())?;
        writer.write_all(&self.rate.to_le_bytes())?;
        writer.write_all(&self.byte_rate.to_le_bytes())?;
        writer.write_all(&self.block_align.to_le_bytes())?;
        writer.write_all(&self.bits_per_sample.to_le_bytes())?;
        writer.write_all(b"data")?;
        writer.write_all(&self.data_len.to_le_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wav_header_declares_ieee_float_32_bit_stereo() {
        let source = AudioBuffer::new(44_100, vec![vec![0.0; 8], vec![0.0; 8]]);
        let mut bytes = Vec::new();
        write(&mut bytes, &source).expect("write the WAV");

        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(&bytes[12..16], b"fmt ");
        assert_eq!(u32::from_le_bytes(bytes[16..20].try_into().unwrap()), 16);
        assert_eq!(
            u16::from_le_bytes(bytes[20..22].try_into().unwrap()),
            WAVE_FORMAT_IEEE_FLOAT
        );
        assert_eq!(
            u16::from_le_bytes(bytes[22..24].try_into().unwrap()),
            2,
            "channels"
        );
        assert_eq!(
            u32::from_le_bytes(bytes[24..28].try_into().unwrap()),
            44_100,
            "rate"
        );
        assert_eq!(
            u32::from_le_bytes(bytes[28..32].try_into().unwrap()),
            44_100 * 8,
            "byte rate"
        );
        assert_eq!(
            u16::from_le_bytes(bytes[32..34].try_into().unwrap()),
            8,
            "block align"
        );
        assert_eq!(
            u16::from_le_bytes(bytes[34..36].try_into().unwrap()),
            32,
            "bits"
        );
        assert_eq!(&bytes[36..40], b"data");
        assert_eq!(
            u32::from_le_bytes(bytes[40..44].try_into().unwrap()),
            8 * 8,
            "data length"
        );
        assert_eq!(bytes.len(), 44 + 8 * 8);
    }

    #[test]
    fn a_wav_keeps_a_sample_outside_the_unit_range() {
        let source = AudioBuffer::new(44_100, vec![vec![2.5]]);
        let mut bytes = Vec::new();
        write(&mut bytes, &source).expect("write the WAV");
        let stored = f32::from_le_bytes(bytes[44..48].try_into().unwrap());
        assert_eq!(stored, 2.5);
    }
}
