//! Opus writing: 48 kHz, 256 kb/s VBR, in Ogg, with the libopus from `opusic-c`.
//!
//! The input is resampled to 48 kHz, the only rate libopus codes. The encoder writes one 20 ms
//! frame per Ogg page. The `OpusHead` pre-skip is the encoder's look-ahead, and the final page's
//! granule is the playable sample count plus that pre-skip, so a decoder returns the input length
//! (RFC 7845 sections 4.2, 4.4, and 7).

use std::io::Write;

use opusic_c::{Application, Bitrate, Channels, Encoder, SampleRate};

use crate::decode::AudioBuffer;

use super::ogg::{PageWriter, FLAG_BEGIN_OF_STREAM, FLAG_END_OF_STREAM};
use super::{EncodeFailure, EncodeOutcome, WriteResult};

/// Opus codes every stream at 48 kHz.
const OUTPUT_RATE: u32 = 48_000;

/// A 20 ms frame, at 48 kHz.
const FRAME_SAMPLES: usize = OUTPUT_RATE as usize / 50;

/// The target bit rate, in bits per second.
const BITRATE: u32 = 256_000;

/// The Ogg stream serial number. Any value is legal.
const SERIAL: u32 = 0x4d45_4c4f;

/// The output buffer for one packet, with margin over the largest useful 20 ms stereo packet.
const MAX_PACKET: usize = 8_192;

pub(super) fn write(writer: &mut dyn Write, source: &AudioBuffer) -> WriteResult {
    let resampled = source.resample_to(OUTPUT_RATE);
    let channels = resampled.channel_count();
    let opus_channels = match channels {
        1 => Channels::Mono,
        2 => Channels::Stereo,
        other => {
            return Err(EncodeFailure::Message(format!(
                "the Opus encoder writes one or two channels, not {other}"
            )))
        }
    };

    let mut encoder = Encoder::new(opus_channels, SampleRate::Hz48000, Application::Audio)
        .map_err(|error| {
            EncodeFailure::Message(format!("build the Opus encoder: {}", error.message()))
        })?;
    encoder
        .set_bitrate(Bitrate::Value(BITRATE))
        .map_err(|error| {
            EncodeFailure::Message(format!("set the Opus bit rate: {}", error.message()))
        })?;
    encoder
        .set_vbr(true)
        .map_err(|error| EncodeFailure::Message(format!("enable Opus VBR: {}", error.message())))?;

    let look_ahead = encoder.get_look_ahead().map_err(|error| {
        EncodeFailure::Message(format!("read the Opus look-ahead: {}", error.message()))
    })?;
    let pre_skip = u16::try_from(look_ahead).map_err(|_| {
        EncodeFailure::Message("the Opus look-ahead does not fit the header".to_string())
    })?;

    let frames = resampled.frames();
    let playable = frames + usize::from(pre_skip);
    let final_granule = playable as u64;
    let packet_count = playable.div_ceil(FRAME_SAMPLES);

    let mut pages = PageWriter::new(writer, SERIAL);
    pages.write_packet(
        &opus_head(channels as u8, pre_skip),
        0,
        FLAG_BEGIN_OF_STREAM,
    )?;
    pages.write_packet(&opus_tags(), 0, 0)?;

    let mut interleaved = vec![0f32; FRAME_SAMPLES * channels];
    let mut packet = vec![0u8; MAX_PACKET];
    for index in 0..packet_count {
        let start = index * FRAME_SAMPLES;
        for (offset, sample) in interleaved.iter_mut().enumerate() {
            let frame = start + offset / channels;
            *sample = if frame < frames {
                resampled.channels()[offset % channels][frame]
            } else {
                0.0
            };
        }
        let length = encoder
            .encode_float_to_slice(&interleaved, &mut packet)
            .map_err(|error| {
                EncodeFailure::Message(format!("encode an Opus frame: {}", error.message()))
            })?;
        let granule = (((index + 1) * FRAME_SAMPLES) as u64).min(final_granule);
        let flags = if index + 1 == packet_count {
            FLAG_END_OF_STREAM
        } else {
            0
        };
        pages.write_packet(&packet[..length], granule, flags)?;
    }
    Ok(EncodeOutcome { clamped: 0 })
}

/// The `OpusHead` identification header (RFC 7845 section 5.1). Mapping family 0 is unpacked mono
/// or stereo, so the header is exactly 19 bytes and carries no channel mapping table.
fn opus_head(channels: u8, pre_skip: u16) -> Vec<u8> {
    let mut head = Vec::with_capacity(19);
    head.extend_from_slice(b"OpusHead");
    head.push(1); // version
    head.push(channels);
    head.extend_from_slice(&pre_skip.to_le_bytes());
    head.extend_from_slice(&OUTPUT_RATE.to_le_bytes()); // input sample rate
    head.extend_from_slice(&0i16.to_le_bytes()); // output gain
    head.push(0); // channel mapping family
    head
}

/// The `OpusTags` comment header (RFC 7845 section 5.2), with the vendor string and no comments.
/// Task 15 adds the source tags here.
fn opus_tags() -> Vec<u8> {
    let vendor = concat!("melomae ", env!("CARGO_PKG_VERSION"));
    let mut tags = Vec::with_capacity(16 + vendor.len());
    tags.extend_from_slice(b"OpusTags");
    tags.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    tags.extend_from_slice(vendor.as_bytes());
    tags.extend_from_slice(&0u32.to_le_bytes());
    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_opus_head_has_the_expected_fields() {
        let head = opus_head(2, 312);
        assert_eq!(head.len(), 19, "mapping family 0 has no mapping table");
        assert_eq!(&head[..8], b"OpusHead");
        assert_eq!(head[8], 1, "version");
        assert_eq!(head[9], 2, "channels");
        assert_eq!(u16::from_le_bytes([head[10], head[11]]), 312, "pre-skip");
        assert_eq!(
            u32::from_le_bytes(head[12..16].try_into().unwrap()),
            48_000,
            "input sample rate"
        );
        assert_eq!(i16::from_le_bytes([head[16], head[17]]), 0, "output gain");
        assert_eq!(head[18], 0, "channel mapping family");
    }

    #[test]
    fn the_opus_tags_hold_the_vendor_and_no_comments() {
        let vendor = concat!("melomae ", env!("CARGO_PKG_VERSION"));
        let tags = opus_tags();
        assert_eq!(&tags[..8], b"OpusTags");
        let vendor_len = u32::from_le_bytes(tags[8..12].try_into().unwrap()) as usize;
        assert_eq!(&tags[12..12 + vendor_len], vendor.as_bytes());
        let comments =
            u32::from_le_bytes(tags[12 + vendor_len..16 + vendor_len].try_into().unwrap());
        assert_eq!(comments, 0);
        assert_eq!(tags.len(), 16 + vendor_len);
    }
}
