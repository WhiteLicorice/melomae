//! A minimal Matroska/WebM reader for the Opus track's `CodecDelay` and `DiscardPadding`.
//!
//! Symphonia's Matroska reader builds untrimmed packets: it shifts packet timestamps by
//! `CodecDelay` but never turns it into a start trim, and it ignores `DiscardPadding` entirely. So
//! the decoder's pre-skip and the encoder's trailing padding would both leak into the output. This
//! module reads only those two elements, so the decoded length is exact.

use std::fs;
use std::path::Path;

const ID_SEGMENT: u32 = 0x1853_8067;
const ID_TRACKS: u32 = 0x1654_AE6B;
const ID_TRACK_ENTRY: u32 = 0xAE;
const ID_TRACK_NUMBER: u32 = 0xD7;
const ID_CODEC_ID: u32 = 0x86;
const ID_CODEC_DELAY: u32 = 0x56AA;
const ID_CLUSTER: u32 = 0x1F43_B675;
const ID_BLOCK_GROUP: u32 = 0xA0;
const ID_BLOCK: u32 = 0xA1;
const ID_DISCARD_PADDING: u32 = 0x75A2;

/// The trim a Matroska Opus track asks for, in samples at the codec's 48 kHz rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OpusTrim {
    /// The encoder delay from `CodecDelay`.
    pub pre_skip: usize,
    /// The trailing padding from every `DiscardPadding` of the audio track.
    pub end_padding: usize,
}

/// Read the Opus trim from a Matroska/WebM file, or `None` when the file is not Matroska or holds
/// no Opus track.
pub(crate) fn opus_trim(path: &Path, sample_rate: u32) -> Option<OpusTrim> {
    let data = fs::read(path).ok()?;
    if !data.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return None;
    }

    let mut state = State::default();
    // The EBML header is a master element; the segment follows it.
    for (id, body) in children(&data) {
        if id == ID_SEGMENT {
            walk_segment(body, &mut state, sample_rate);
        }
    }

    let track = state.audio_track?;
    let _ = track;
    Some(OpusTrim {
        pre_skip: ns_to_samples(state.codec_delay_ns, sample_rate),
        end_padding: ns_to_samples(state.discard_ns.unsigned_abs(), sample_rate),
    })
}

#[derive(Default)]
struct State {
    audio_track: Option<u64>,
    codec_delay_ns: u64,
    discard_ns: i64,
}

fn walk_segment(data: &[u8], state: &mut State, sample_rate: u32) {
    for (id, body) in children(data) {
        match id {
            ID_TRACKS => walk_tracks(body, state),
            ID_CLUSTER => walk_cluster(body, state, sample_rate),
            _ => {}
        }
    }
}

fn walk_tracks(data: &[u8], state: &mut State) {
    for (id, body) in children(data) {
        if id != ID_TRACK_ENTRY {
            continue;
        }
        let mut number = None;
        let mut is_opus = false;
        let mut delay_ns = 0u64;
        for (child, value) in children(body) {
            match child {
                ID_TRACK_NUMBER => number = read_uint(value),
                ID_CODEC_ID => is_opus = value == b"A_OPUS",
                ID_CODEC_DELAY => delay_ns = read_uint(value).unwrap_or(0),
                _ => {}
            }
        }
        if is_opus {
            if let Some(number) = number {
                state.audio_track = Some(number);
                state.codec_delay_ns = delay_ns;
            }
        }
    }
}

fn walk_cluster(data: &[u8], state: &mut State, sample_rate: u32) {
    for (id, body) in children(data) {
        if id != ID_BLOCK_GROUP {
            continue;
        }
        let mut block_track = None;
        let mut discard_ns = 0i64;
        for (child, value) in children(body) {
            match child {
                ID_BLOCK => block_track = read_block_track(value),
                ID_DISCARD_PADDING => {
                    discard_ns = read_int(value).unwrap_or(0);
                    let _ = sample_rate;
                }
                _ => {}
            }
        }
        if block_track == state.audio_track {
            state.discard_ns += discard_ns;
        }
    }
}

/// The direct children of an EBML master element.
fn children(data: &[u8]) -> impl Iterator<Item = (u32, &[u8])> {
    let mut offset = 0usize;
    std::iter::from_fn(move || {
        let (id, id_len) = read_id(data, offset)?;
        let (size, size_len) = read_size(data, offset + id_len)?;
        let body_start = offset + id_len + size_len;
        // An unknown size means the element runs to the end of its parent.
        let body_len = match size {
            Some(size) => usize::try_from(size).ok()?,
            None => data.len().checked_sub(body_start)?,
        };
        let body = data.get(body_start..body_start + body_len)?;
        offset = body_start + body_len;
        Some((id, body))
    })
}

/// Read an element ID, returning its value (with the length marker) and byte length.
fn read_id(data: &[u8], pos: usize) -> Option<(u32, usize)> {
    let first = *data.get(pos)?;
    let len = first.leading_zeros() as usize + 1;
    if len > 4 {
        return None;
    }
    let bytes = data.get(pos..pos + len)?;
    let mut value = 0u32;
    for byte in bytes {
        value = (value << 8) | u32::from(*byte);
    }
    Some((value, len))
}

/// Read an element size, returning its value and byte length. `None` size values mean "unknown".
fn read_size(data: &[u8], pos: usize) -> Option<(Option<u64>, usize)> {
    let first = *data.get(pos)?;
    if first == 0 {
        return None;
    }
    let len = first.leading_zeros() as usize + 1;
    if len > 8 {
        return None;
    }
    let bytes = data.get(pos..pos + len)?;
    let marker = (1u16 << (8 - len)) - 1;
    let mut value = u64::from(first) & u64::from(marker as u8);
    let mut all_ones = value == u64::from(marker as u8);
    for byte in &bytes[1..] {
        value = (value << 8) | u64::from(*byte);
        if *byte != 0xFF {
            all_ones = false;
        }
    }
    Some((if all_ones { None } else { Some(value) }, len))
}

/// Read an unsigned big-endian integer of up to eight bytes.
fn read_uint(data: &[u8]) -> Option<u64> {
    if data.is_empty() || data.len() > 8 {
        return None;
    }
    Some(
        data.iter()
            .fold(0u64, |acc, byte| (acc << 8) | u64::from(*byte)),
    )
}

/// Read a signed big-endian integer of up to eight bytes.
fn read_int(data: &[u8]) -> Option<i64> {
    let value = read_uint(data)?;
    let bits = data.len() * 8;
    if bits < 64 && value & (1 << (bits - 1)) != 0 {
        Some((value | (!0u64 << bits)) as i64)
    } else {
        Some(value as i64)
    }
}

/// Read the track number at the start of a `Block` payload.
fn read_block_track(data: &[u8]) -> Option<u64> {
    let first = *data.first()?;
    let len = first.leading_zeros() as usize + 1;
    if len > 8 {
        return None;
    }
    let bytes = data.get(..len)?;
    let marker = (1u16 << (8 - len)) - 1;
    let mut value = u64::from(first) & u64::from(marker as u8);
    for byte in &bytes[1..] {
        value = (value << 8) | u64::from(*byte);
    }
    Some(value)
}

/// Convert a nanosecond duration to samples, rounding to nearest.
fn ns_to_samples(nanoseconds: u64, sample_rate: u32) -> usize {
    ((u128::from(nanoseconds) * u128::from(sample_rate) + 500_000_000) / 1_000_000_000) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encode an element ID (with its length marker).
    fn id(value: u32) -> Vec<u8> {
        let bytes = value.to_be_bytes();
        let first = bytes.iter().position(|byte| *byte != 0).unwrap_or(3);
        bytes[first..].to_vec()
    }

    /// Encode an element size as a one-byte vint (sizes under 127).
    fn size(value: usize) -> Vec<u8> {
        assert!(value < 127);
        vec![0x80 | value as u8]
    }

    fn element(id_value: u32, body: &[u8]) -> Vec<u8> {
        let mut out = id(id_value);
        out.extend_from_slice(&size(body.len()));
        out.extend_from_slice(body);
        out
    }

    fn uint(value: u64) -> Vec<u8> {
        let bytes = value.to_be_bytes();
        let first = bytes.iter().position(|byte| *byte != 0).unwrap_or(7);
        bytes[first..].to_vec()
    }

    /// A file with one Opus track (number 1, CodecDelay 6.5 ms) and one BlockGroup whose Block is
    /// on that track and whose DiscardPadding is 13.5 ms.
    fn webm() -> Vec<u8> {
        let mut entry = Vec::new();
        entry.extend_from_slice(&element(ID_TRACK_NUMBER, &uint(1)));
        entry.extend_from_slice(&element(ID_CODEC_ID, b"A_OPUS"));
        entry.extend_from_slice(&element(ID_CODEC_DELAY, &uint(6_500_000)));

        let mut tracks = Vec::new();
        tracks.extend_from_slice(&element(ID_TRACK_ENTRY, &entry));

        // A Block starts with the track number as a vint (track 1 -> 0x81).
        let mut group = Vec::new();
        group.extend_from_slice(&element(ID_BLOCK, &[0x81, 0x00, 0x00]));
        group.extend_from_slice(&element(ID_DISCARD_PADDING, &[0x00, 0xCE, 0x11, 0x40]));

        let mut cluster = Vec::new();
        cluster.extend_from_slice(&element(ID_BLOCK_GROUP, &group));

        let mut segment = Vec::new();
        segment.extend_from_slice(&element(ID_TRACKS, &tracks));
        segment.extend_from_slice(&element(ID_CLUSTER, &cluster));

        let mut out = vec![0x1A, 0x45, 0xDF, 0xA3, 0x80];
        out.extend_from_slice(&element(ID_SEGMENT, &segment));
        out
    }

    fn write_temp(name: &str, data: &[u8]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join("melomae-decode-tests").join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, data).unwrap();
        path
    }

    #[test]
    fn reads_codec_delay_and_discard_padding() {
        let path = write_temp("mkv_opus_trim.webm", &webm());
        let trim = opus_trim(&path, 48_000).expect("an Opus trim");
        assert_eq!(trim.pre_skip, 312);
        assert_eq!(trim.end_padding, 648);
    }

    #[test]
    fn ignores_a_non_matroska_file() {
        let path = write_temp("mkv_not_matroska.bin", b"no EBML magic here");
        assert_eq!(opus_trim(&path, 48_000), None);
    }

    #[test]
    fn ns_to_samples_rounds_to_nearest() {
        assert_eq!(ns_to_samples(6_500_000, 48_000), 312);
        assert_eq!(ns_to_samples(13_500_000, 48_000), 648);
        assert_eq!(ns_to_samples(0, 48_000), 0);
    }
}
