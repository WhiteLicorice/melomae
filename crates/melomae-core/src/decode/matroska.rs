//! A streaming Matroska/WebM reader for the Opus track's `CodecDelay` and `DiscardPadding`.
//!
//! Symphonia's Matroska reader builds untrimmed packets: it shifts packet timestamps by
//! `CodecDelay` but never turns it into a start trim, and it ignores `DiscardPadding` entirely. So
//! the decoder's pre-skip and the encoder's trailing padding would both leak into the output. This
//! module reads only those two elements, so the decoded length is exact.
//!
//! Nothing here reads a whole element body into memory. Element headers are read through a
//! `Read + Seek` reader, and every payload this reader does not need is skipped by seeking. The
//! only allocations scale with the track count and the number of audio blocks, not with the file
//! size or the audio payload.

use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use super::{DecodeError, DecodeErrorKind};

const ID_SEGMENT: u32 = 0x1853_8067;
const ID_TRACKS: u32 = 0x1654_AE6B;
const ID_CLUSTER: u32 = 0x1F43_B675;
const ID_VOID: u32 = 0xEC;
const ID_CRC32: u32 = 0xBF;

const ID_TRACK_ENTRY: u32 = 0xAE;
const ID_TRACK_NUMBER: u32 = 0xD7;
const ID_CODEC_ID: u32 = 0x86;
const ID_CODEC_DELAY: u32 = 0x56AA;

const ID_CLUSTER_TIMESTAMP: u32 = 0xE7;
const ID_POSITION: u32 = 0xA7;
const ID_PREV_SIZE: u32 = 0xAB;
const ID_SIMPLE_BLOCK: u32 = 0xA3;
const ID_BLOCK_GROUP: u32 = 0xA0;
const ID_ENCRYPTED_BLOCK: u32 = 0xAF;
const ID_SILENT_TRACKS: u32 = 0x5854;
const ID_BLOCK: u32 = 0xA1;
const ID_DISCARD_PADDING: u32 = 0x75A2;

/// The longest Opus packet is 120 ms, so a block of `frames` packets cannot hold more samples
/// than this. It bounds a `DiscardPadding` before the padding is applied to decoded audio.
const OPUS_MAX_FRAMES_PER_PACKET: usize = 5760;

/// The trim a single Matroska audio block asks for, in samples at the codec's 48 kHz rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BlockTrim {
    /// How many Opus packets (lace frames) the block holds.
    pub frames: usize,
    /// Padding to discard. A positive value is discarded from the end of the block, a negative
    /// value from its start, as the Matroska element defines it.
    pub padding: i64,
}

/// The trim a Matroska Opus track asks for, in samples at the codec's 48 kHz rate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpusTrim {
    /// The encoder delay from `CodecDelay`, applied once at the start of the stream.
    pub pre_skip: usize,
    /// One entry per audio-track block, in file order.
    pub blocks: Vec<BlockTrim>,
}

/// Read the Opus trim for `track` from a Matroska/WebM stream.
///
/// Returns `Ok(None)` when the stream is not Matroska or holds no block for `track`.
pub(crate) fn opus_trim<R: Read + Seek>(
    path: &Path,
    reader: &mut R,
    track: u64,
    sample_rate: u32,
) -> Result<Option<OpusTrim>, DecodeError> {
    let mut scanner = Scanner::new(reader).map_err(|e| to_error(path, Fail::Io(e)))?;
    scan(path, &mut scanner, track, sample_rate).map_err(|fail| to_error(path, fail))
}

/// The scanner carries the current byte position so element bounds can be checked and payloads
/// skipped by seeking.
struct Scanner<R> {
    inner: R,
    pos: u64,
}

impl<R: Read + Seek> Scanner<R> {
    fn new(mut inner: R) -> io::Result<Self> {
        let pos = inner.stream_position()?;
        Ok(Self { inner, pos })
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Fail> {
        let read = self.inner.read(buf)?;
        self.pos += read as u64;
        Ok(read)
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Fail> {
        let mut filled = 0;
        while filled < buf.len() {
            let read = self.read(&mut buf[filled..])?;
            if read == 0 {
                return Err(Fail::Eof);
            }
            filled += read;
        }
        Ok(())
    }

    fn byte(&mut self) -> Result<u8, Fail> {
        let mut one = [0u8; 1];
        if self.read(&mut one)? == 0 {
            return Err(Fail::Eof);
        }
        Ok(one[0])
    }

    fn seek(&mut self, pos: u64) -> Result<(), Fail> {
        self.inner.seek(SeekFrom::Start(pos))?;
        self.pos = pos;
        Ok(())
    }
}

/// A fatal scan failure. `Eof` means the stream ended inside an element, which is a truncation.
enum Fail {
    Eof,
    Bad(&'static str),
    Io(io::Error),
}

impl From<io::Error> for Fail {
    fn from(error: io::Error) -> Self {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            Fail::Eof
        } else {
            Fail::Io(error)
        }
    }
}

fn to_error(path: &Path, fail: Fail) -> DecodeError {
    match fail {
        Fail::Eof => DecodeError::new(
            DecodeErrorKind::Malformed,
            path,
            "the Matroska stream ends inside an element",
        ),
        Fail::Bad(reason) => DecodeError::new(DecodeErrorKind::Malformed, path, reason),
        Fail::Io(error) => DecodeError::new(DecodeErrorKind::Io, path, error.to_string()),
    }
}

#[derive(Default)]
struct State {
    track: u64,
    found_track: bool,
    codec_delay_ns: u64,
    blocks: Vec<BlockTrim>,
    rate: u32,
}

fn scan<R: Read + Seek>(
    path: &Path,
    scanner: &mut Scanner<R>,
    track: u64,
    rate: u32,
) -> Result<Option<OpusTrim>, Fail> {
    let mut magic = [0u8; 4];
    let read = scanner.read(&mut magic)?;
    if read < 4 || magic != [0x1A, 0x45, 0xDF, 0xA3] {
        return Ok(None);
    }

    let mut state = State {
        track,
        rate,
        ..State::default()
    };
    scanner.seek(0)?;

    while let Some((id, _)) = read_id(scanner)? {
        let size = read_size(scanner)?;
        let body_start = scanner.pos;
        let body_end = element_end(body_start, size)?;
        if id == ID_SEGMENT {
            parse_segment(scanner, body_end, &mut state)?;
            break;
        }
        skip(scanner, body_start, body_end)?;
    }

    if !state.found_track {
        return Ok(None);
    }
    let _ = path;
    let pre_skip =
        ns_to_samples(state.codec_delay_ns, state.rate).ok_or(Fail::Bad("CodecDelay is out of range"))?;
    let blocks = state.blocks;
    Ok(Some(OpusTrim { pre_skip, blocks }))
}

/// The absolute end of an element body, or `None` when the size is the unknown marker.
fn element_end(body_start: u64, size: Option<u64>) -> Result<Option<u64>, Fail> {
    match size {
        Some(size) => Ok(Some(
            body_start
                .checked_add(size)
                .ok_or(Fail::Bad("an element size overflows the stream"))?,
        )),
        None => Ok(None),
    }
}

fn parse_segment<R: Read + Seek>(
    scanner: &mut Scanner<R>,
    end: Option<u64>,
    state: &mut State,
) -> Result<(), Fail> {
    loop {
        if let Some(end) = end {
            if scanner.pos >= end {
                return Ok(());
            }
        }
        let Some((id, _)) = read_id(scanner)? else {
            return Ok(());
        };
        let size = read_size(scanner)?;
        let body_start = scanner.pos;
        let child_end = element_end(body_start, size)?;
        match id {
            ID_TRACKS => parse_tracks(scanner, child_end, state)?,
            ID_CLUSTER => parse_cluster(scanner, child_end, state)?,
            _ => skip(scanner, body_start, child_end)?,
        }
    }
}

fn parse_tracks<R: Read + Seek>(
    scanner: &mut Scanner<R>,
    end: Option<u64>,
    state: &mut State,
) -> Result<(), Fail> {
    let end = end.ok_or(Fail::Bad("the Tracks element has no size"))?;
    while scanner.pos < end {
        let (id, _) = read_id(scanner)?.ok_or(Fail::Eof)?;
        let size = read_size(scanner)?;
        let body_start = scanner.pos;
        let child_end = element_end(body_start, size)?;
        if id == ID_TRACK_ENTRY {
            parse_track_entry(scanner, child_end.ok_or(Fail::Bad("a track entry has no size"))?, state)?;
        } else {
            skip(scanner, body_start, child_end)?;
        }
    }
    Ok(())
}

fn parse_track_entry<R: Read + Seek>(
    scanner: &mut Scanner<R>,
    end: u64,
    state: &mut State,
) -> Result<(), Fail> {
    let mut number = None;
    let mut is_opus = false;
    let mut delay_ns = 0u64;
    while scanner.pos < end {
        let (id, _) = read_id(scanner)?.ok_or(Fail::Eof)?;
        let size = read_size(scanner)?;
        let body_start = scanner.pos;
        let child_end = element_end(body_start, size)?;
        let child_end = child_end.ok_or(Fail::Bad("a track entry child has no size"))?;
        match id {
            ID_TRACK_NUMBER => number = read_uint(scanner, child_end)?,
            ID_CODEC_ID => is_opus = read_bytes(scanner, child_end)? == b"A_OPUS",
            ID_CODEC_DELAY => delay_ns = read_uint(scanner, child_end)?.unwrap_or(0),
            _ => {}
        }
        scanner.seek(child_end)?;
    }
    if is_opus && number == Some(state.track) {
        state.found_track = true;
        state.codec_delay_ns = delay_ns;
    }
    Ok(())
}

fn parse_cluster<R: Read + Seek>(
    scanner: &mut Scanner<R>,
    end: Option<u64>,
    state: &mut State,
) -> Result<(), Fail> {
    loop {
        if let Some(end) = end {
            if scanner.pos >= end {
                return Ok(());
            }
        }
        let id_start = scanner.pos;
        let Some((id, _)) = read_id(scanner)? else {
            // A clean EOF ends an unknown-size cluster. A finite cluster that ends before its
            // declared size is truncated.
            return if end.is_none() { Ok(()) } else { Err(Fail::Eof) };
        };
        // An unknown-size cluster runs until an element that is not one of its children appears,
        // which is the RFC 8794 section 6.2 termination rule.
        if end.is_none() && !is_cluster_child(id) {
            scanner.seek(id_start)?;
            return Ok(());
        }
        let size = read_size(scanner)?;
        let body_start = scanner.pos;
        let child_end = element_end(body_start, size)?;
        match id {
            ID_SIMPLE_BLOCK => {
                // A SimpleBlock carries no `DiscardPadding`, but its frame count is needed so a
                // later block's trim lands on the right decoded samples.
                let end = child_end.ok_or(Fail::Bad("a simple block has no size"))?;
                let (block_track, frames) = read_block_header(scanner, end)?;
                if block_track == Some(state.track) {
                    state.blocks.push(BlockTrim { frames, padding: 0 });
                }
                scanner.seek(end)?;
            }
            ID_BLOCK_GROUP => parse_block_group(scanner, child_end, state)?,
            _ => skip(scanner, body_start, child_end)?,
        }
    }
}

/// Whether `id` is a direct child of a Cluster, used to terminate an unknown-size Cluster.
fn is_cluster_child(id: u32) -> bool {
    matches!(
        id,
        ID_CLUSTER_TIMESTAMP | ID_POSITION | ID_PREV_SIZE | ID_SIMPLE_BLOCK | ID_BLOCK_GROUP
            | ID_ENCRYPTED_BLOCK
            | ID_SILENT_TRACKS
            | ID_VOID
            | ID_CRC32
    )
}

fn parse_block_group<R: Read + Seek>(
    scanner: &mut Scanner<R>,
    end: Option<u64>,
    state: &mut State,
) -> Result<(), Fail> {
    let end = end.ok_or(Fail::Bad("a block group has no size"))?;
    let mut track = None;
    let mut frames = 1usize;
    let mut padding_ns = 0i64;
    while scanner.pos < end {
        let (id, _) = read_id(scanner)?.ok_or(Fail::Eof)?;
        let size = read_size(scanner)?;
        let body_start = scanner.pos;
        let child_end = element_end(body_start, size)?;
        let child_end = child_end.ok_or(Fail::Bad("a block group child has no size"))?;
        match id {
            ID_BLOCK => {
                let (block_track, block_frames) = read_block_header(scanner, child_end)?;
                track = block_track;
                frames = block_frames;
            }
            ID_DISCARD_PADDING => padding_ns = read_int(scanner, child_end)?.unwrap_or(0),
            _ => {}
        }
        scanner.seek(child_end)?;
    }
    if track == Some(state.track) {
        let padding = ns_to_samples_signed(padding_ns, state.rate)
            .ok_or(Fail::Bad("a DiscardPadding is out of range"))?;
        let max_samples = frames.saturating_mul(OPUS_MAX_FRAMES_PER_PACKET) as u64;
        if padding.unsigned_abs() > max_samples {
            return Err(Fail::Bad("a DiscardPadding is larger than its block"));
        }
        state.blocks.push(BlockTrim { frames, padding });
    }
    Ok(())
}

/// Skip an element body by seeking to its end. An unknown-size leaf cannot be skipped.
fn skip<R: Read + Seek>(
    scanner: &mut Scanner<R>,
    body_start: u64,
    end: Option<u64>,
) -> Result<(), Fail> {
    match end {
        Some(end) => {
            if end < body_start {
                return Err(Fail::Bad("an element size is smaller than its header"));
            }
            scanner.seek(end)
        }
        None => Err(Fail::Bad("an unknown-size element is not a master")),
    }
}

/// Read an element ID, returning its value (with the length marker) and byte length.
fn read_id<R: Read + Seek>(scanner: &mut Scanner<R>) -> Result<Option<(u32, usize)>, Fail> {
    let first = match scanner.read(&mut [0u8; 1])? {
        0 => return Ok(None),
        _ => {
            // `read` already advanced the position; re-read the byte from the buffer.
            scanner.seek(scanner.pos - 1)?;
            scanner.byte()?
        }
    };
    if first == 0 {
        return Err(Fail::Bad("an element id is zero"));
    }
    let len = first.leading_zeros() as usize + 1;
    if len > 4 {
        return Err(Fail::Bad("an element id is longer than four bytes"));
    }
    let mut value = u32::from(first);
    let mut rest = [0u8; 3];
    if len > 1 {
        scanner.read_exact(&mut rest[..len - 1])?;
        for byte in &rest[..len - 1] {
            value = (value << 8) | u32::from(*byte);
        }
    }
    Ok(Some((value, len)))
}

/// Read an element size. `None` is the unknown-size marker.
fn read_size<R: Read + Seek>(scanner: &mut Scanner<R>) -> Result<Option<u64>, Fail> {
    let first = scanner.byte()?;
    if first == 0 {
        return Err(Fail::Bad("an element size is zero length"));
    }
    let len = first.leading_zeros() as usize + 1;
    if len > 8 {
        return Err(Fail::Bad("an element size is longer than eight bytes"));
    }
    let marker = (1u16 << (8 - len)) - 1;
    let mut value = u64::from(first) & u64::from(marker as u8);
    let mut all_ones = value == u64::from(marker as u8);
    for _ in 1..len {
        let byte = scanner.byte()?;
        value = (value << 8) | u64::from(byte);
        if byte != 0xFF {
            all_ones = false;
        }
    }
    Ok(if all_ones { None } else { Some(value) })
}

/// Read the track number, the timecode, the flags, and the lace count of a `Block` payload.
///
/// Only the first few bytes are consumed; the audio payload is skipped by the caller.
fn read_block_header<R: Read + Seek>(
    scanner: &mut Scanner<R>,
    end: u64,
) -> Result<(Option<u64>, usize), Fail> {
    if end.saturating_sub(scanner.pos) < 4 {
        return Ok((None, 1));
    }
    let track = read_vint_number(scanner)?;
    let mut header = [0u8; 3];
    scanner.read_exact(&mut header)?;
    let flags = header[2];
    let lacing = (flags >> 1) & 0x03;
    let frames = if lacing == 0 {
        1
    } else {
        usize::from(scanner.byte()?) + 1
    };
    Ok((track, frames))
}

/// Read the track number vint at the start of a `Block` payload.
fn read_vint_number<R: Read + Seek>(scanner: &mut Scanner<R>) -> Result<Option<u64>, Fail> {
    let first = scanner.byte()?;
    if first == 0 {
        return Err(Fail::Bad("a block track number is zero"));
    }
    let len = first.leading_zeros() as usize + 1;
    if len > 8 {
        return Err(Fail::Bad("a block track number is too long"));
    }
    let marker = (1u16 << (8 - len)) - 1;
    let mut value = u64::from(first) & u64::from(marker as u8);
    for _ in 1..len {
        value = (value << 8) | u64::from(scanner.byte()?);
    }
    Ok(Some(value))
}

/// Read an unsigned big-endian integer of up to eight bytes from `[pos, end)`.
fn read_uint<R: Read + Seek>(scanner: &mut Scanner<R>, end: u64) -> Result<Option<u64>, Fail> {
    let len = end.saturating_sub(scanner.pos);
    if len == 0 || len > 8 {
        return Ok(None);
    }
    let mut buf = [0u8; 8];
    scanner.read_exact(&mut buf[..len as usize])?;
    Ok(Some(
        buf[..len as usize]
            .iter()
            .fold(0u64, |acc, byte| (acc << 8) | u64::from(*byte)),
    ))
}

/// Read a signed big-endian integer of up to eight bytes from `[pos, end)`.
fn read_int<R: Read + Seek>(scanner: &mut Scanner<R>, end: u64) -> Result<Option<i64>, Fail> {
    let len = end.saturating_sub(scanner.pos);
    if len == 0 || len > 8 {
        return Ok(None);
    }
    let mut buf = [0u8; 8];
    scanner.read_exact(&mut buf[..len as usize])?;
    let value = buf[..len as usize]
        .iter()
        .fold(0u64, |acc, byte| (acc << 8) | u64::from(*byte));
    let bits = len as u32 * 8;
    if bits < 64 && value & (1u64 << (bits - 1)) != 0 {
        Ok(Some((value | (!0u64 << bits)) as i64))
    } else {
        Ok(Some(value as i64))
    }
}

/// Read up to 64 bytes from `[pos, end)`.
fn read_bytes<R: Read + Seek>(scanner: &mut Scanner<R>, end: u64) -> Result<Vec<u8>, Fail> {
    let len = end.saturating_sub(scanner.pos);
    if len > 64 {
        return Err(Fail::Bad("a CodecID is longer than expected"));
    }
    let mut buf = vec![0u8; len as usize];
    scanner.read_exact(&mut buf)?;
    Ok(buf)
}

/// Convert a nanosecond duration to samples, rounding to nearest. `None` on overflow.
fn ns_to_samples(nanoseconds: u64, sample_rate: u32) -> Option<usize> {
    let scaled = u128::from(nanoseconds) * u128::from(sample_rate) + 500_000_000;
    usize::try_from(scaled / 1_000_000_000).ok()
}

/// Convert a signed nanosecond duration to samples, rounding to nearest. `None` on overflow.
fn ns_to_samples_signed(nanoseconds: i64, sample_rate: u32) -> Option<i64> {
    let scaled = nanoseconds.unsigned_abs() as u128 * u128::from(sample_rate) + 500_000_000;
    let samples = i64::try_from(scaled / 1_000_000_000).ok()?;
    Some(if nanoseconds < 0 { -samples } else { samples })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Read a trim from an in-memory byte stream.
    fn read(data: &[u8], track: u64) -> Result<Option<OpusTrim>, DecodeError> {
        let mut cursor = Cursor::new(data.to_vec());
        opus_trim(Path::new("memory.webm"), &mut cursor, track, 48_000)
    }

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

    fn track_entry(number: u64, delay_ns: u64) -> Vec<u8> {
        let mut entry = Vec::new();
        entry.extend_from_slice(&element(ID_TRACK_NUMBER, &uint(number)));
        entry.extend_from_slice(&element(ID_CODEC_ID, b"A_OPUS"));
        entry.extend_from_slice(&element(ID_CODEC_DELAY, &uint(delay_ns)));
        entry
    }

    fn block_group(track: u8, padding: i64) -> Vec<u8> {
        let mut group = Vec::new();
        group.extend_from_slice(&element(ID_BLOCK, &[track, 0x00, 0x00, 0x80]));
        group.extend_from_slice(&element(ID_DISCARD_PADDING, &padding.to_be_bytes()));
        group
    }

    /// A file with one Opus track (number 1, CodecDelay 6.5 ms) and one BlockGroup whose
    /// DiscardPadding is 13.5 ms.
    fn webm() -> Vec<u8> {
        let mut tracks = Vec::new();
        tracks.extend_from_slice(&element(ID_TRACK_ENTRY, &track_entry(1, 6_500_000)));

        let mut cluster = Vec::new();
        cluster.extend_from_slice(&element(ID_BLOCK_GROUP, &block_group(0x81, 13_500_000)));

        let mut segment = Vec::new();
        segment.extend_from_slice(&element(ID_TRACKS, &tracks));
        segment.extend_from_slice(&element(ID_CLUSTER, &cluster));

        let mut out = vec![0x1A, 0x45, 0xDF, 0xA3, 0x80];
        out.extend_from_slice(&element(ID_SEGMENT, &segment));
        out
    }

    #[test]
    fn reads_codec_delay_and_discard_padding() {
        let trim = read(&webm(), 1).expect("an Opus trim").expect("some trim");
        assert_eq!(trim.pre_skip, 312);
        assert_eq!(
            trim.blocks,
            vec![BlockTrim {
                frames: 1,
                padding: 648
            }]
        );
    }

    #[test]
    fn ignores_a_non_matroska_file() {
        assert_eq!(read(b"no EBML magic here", 1).unwrap(), None);
    }

    #[test]
    fn reads_only_the_requested_track() {
        let mut tracks = Vec::new();
        tracks.extend_from_slice(&element(ID_TRACK_ENTRY, &track_entry(1, 6_500_000)));
        tracks.extend_from_slice(&element(ID_TRACK_ENTRY, &track_entry(2, 2_500_000)));

        let mut cluster = Vec::new();
        cluster.extend_from_slice(&element(ID_BLOCK_GROUP, &block_group(0x82, 17_500_000)));
        cluster.extend_from_slice(&element(ID_BLOCK_GROUP, &block_group(0x81, 13_500_000)));
        let mut segment = Vec::new();
        segment.extend_from_slice(&element(ID_TRACKS, &tracks));
        segment.extend_from_slice(&element(ID_CLUSTER, &cluster));
        let mut out = vec![0x1A, 0x45, 0xDF, 0xA3, 0x80];
        out.extend_from_slice(&element(ID_SEGMENT, &segment));

        let trim = read(&out, 2).unwrap().unwrap();
        assert_eq!(trim.pre_skip, 120);
        assert_eq!(trim.blocks.len(), 1);
        assert_eq!(trim.blocks[0].padding, 840);
    }

    #[test]
    fn keeps_the_sign_of_discard_padding() {
        let mut tracks = Vec::new();
        tracks.extend_from_slice(&element(ID_TRACK_ENTRY, &track_entry(1, 0)));
        let mut cluster = Vec::new();
        cluster.extend_from_slice(&element(ID_BLOCK_GROUP, &block_group(0x81, -1_000_000)));
        let mut segment = Vec::new();
        segment.extend_from_slice(&element(ID_TRACKS, &tracks));
        segment.extend_from_slice(&element(ID_CLUSTER, &cluster));
        let mut out = vec![0x1A, 0x45, 0xDF, 0xA3, 0x80];
        out.extend_from_slice(&element(ID_SEGMENT, &segment));

        let trim = read(&out, 1).unwrap().unwrap();
        assert_eq!(trim.blocks[0].padding, -48);
    }

    #[test]
    fn reads_laced_block_frame_count() {
        // Flags byte 0x06 selects Xiph lacing; the next byte encodes frames - 1.
        let mut group = Vec::new();
        let mut block = vec![0x81u8, 0x00, 0x00, 0x06, 0x02];
        block.extend_from_slice(&[10, 20, 30]);
        group.extend_from_slice(&element(ID_BLOCK, &block));
        group.extend_from_slice(&element(ID_DISCARD_PADDING, &13_500_000i64.to_be_bytes()));

        let mut tracks = Vec::new();
        tracks.extend_from_slice(&element(ID_TRACK_ENTRY, &track_entry(1, 0)));
        let mut cluster = Vec::new();
        cluster.extend_from_slice(&element(ID_BLOCK_GROUP, &group));
        let mut segment = Vec::new();
        segment.extend_from_slice(&element(ID_TRACKS, &tracks));
        segment.extend_from_slice(&element(ID_CLUSTER, &cluster));
        let mut out = vec![0x1A, 0x45, 0xDF, 0xA3, 0x80];
        out.extend_from_slice(&element(ID_SEGMENT, &segment));

        let trim = read(&out, 1).unwrap().unwrap();
        assert_eq!(trim.blocks[0].frames, 3);
    }

    #[test]
    fn terminates_an_unknown_size_cluster() {
        // An unknown-size cluster followed by a sibling cluster that carries the padding.
        let mut tracks = Vec::new();
        tracks.extend_from_slice(&element(ID_TRACK_ENTRY, &track_entry(1, 0)));

        let mut unknown = Vec::new();
        unknown.extend_from_slice(&element(ID_SIMPLE_BLOCK, &[0x81, 0x00, 0x00, 0x80, 0xAA]));
        let mut unknown_cluster = id(ID_CLUSTER);
        unknown_cluster.extend_from_slice(&[0xFF]); // unknown size
        unknown_cluster.extend_from_slice(&unknown);

        let mut sized = Vec::new();
        sized.extend_from_slice(&element(ID_BLOCK_GROUP, &block_group(0x81, 13_500_000)));

        let mut segment = Vec::new();
        segment.extend_from_slice(&element(ID_TRACKS, &tracks));
        segment.extend_from_slice(&unknown_cluster);
        segment.extend_from_slice(&element(ID_CLUSTER, &sized));

        let mut out = vec![0x1A, 0x45, 0xDF, 0xA3, 0x80];
        out.extend_from_slice(&element(ID_SEGMENT, &segment));

        let trim = read(&out, 1).unwrap().unwrap();
        // The unknown-size cluster's SimpleBlock, then the sized cluster's padded BlockGroup.
        assert_eq!(trim.blocks.len(), 2);
        assert_eq!(trim.blocks[0].padding, 0);
        assert_eq!(trim.blocks[1].padding, 648);
    }

    #[test]
    fn ns_to_samples_rounds_to_nearest() {
        assert_eq!(ns_to_samples(6_500_000, 48_000), Some(312));
        assert_eq!(ns_to_samples(13_500_000, 48_000), Some(648));
        assert_eq!(ns_to_samples(0, 48_000), Some(0));
    }

    #[test]
    fn ns_to_samples_signed_keeps_the_sign() {
        assert_eq!(ns_to_samples_signed(-1_000_000, 48_000), Some(-48));
        assert_eq!(ns_to_samples_signed(1_000_000, 48_000), Some(48));
    }

    /// A file with one Opus track and the given cluster children.
    fn file_with_blocks(blocks: &[Vec<u8>]) -> Vec<u8> {
        let mut tracks = Vec::new();
        tracks.extend_from_slice(&element(ID_TRACK_ENTRY, &track_entry(1, 0)));
        let mut cluster = Vec::new();
        for block in blocks {
            cluster.extend_from_slice(block);
        }
        let mut segment = Vec::new();
        segment.extend_from_slice(&element(ID_TRACKS, &tracks));
        segment.extend_from_slice(&element(ID_CLUSTER, &cluster));
        let mut out = vec![0x1A, 0x45, 0xDF, 0xA3, 0x80];
        out.extend_from_slice(&element(ID_SEGMENT, &segment));
        out
    }

    #[test]
    fn records_simple_blocks_and_a_signed_interior_padding() {
        // A SimpleBlock, then an interior BlockGroup whose DiscardPadding is negative.
        let simple = element(ID_SIMPLE_BLOCK, &[0x81, 0x00, 0x00, 0x80]);
        let group = element(ID_BLOCK_GROUP, &block_group(0x81, -1_000_000));
        let trim = read(&file_with_blocks(&[simple, group]), 1).unwrap().unwrap();
        assert_eq!(
            trim.blocks,
            vec![
                BlockTrim {
                    frames: 1,
                    padding: 0
                },
                BlockTrim {
                    frames: 1,
                    padding: -48
                },
            ]
        );
    }

    #[test]
    fn hostile_discard_padding_returns_an_error() {
        // Two blocks whose DiscardPadding is `i64::MAX` nanoseconds. The old reader added these
        // into one `i64` and overflowed; the reader must return a typed error instead.
        let mut group = Vec::new();
        group.extend_from_slice(&element(ID_BLOCK, &[0x81, 0x00, 0x00, 0x80]));
        group.extend_from_slice(&element(ID_DISCARD_PADDING, &i64::MAX.to_be_bytes()));
        let block = element(ID_BLOCK_GROUP, &group);

        let data = file_with_blocks(&[block.clone(), block]);
        let result = std::panic::catch_unwind(|| read(&data, 1));
        let error = result
            .expect("the reader must not panic on hostile padding")
            .expect_err("hostile padding must return an error");
        assert_eq!(error.kind(), DecodeErrorKind::Malformed);
    }

    /// Encode an element size with the shortest vint that fits, for sizes over 126.
    fn size_vint(value: usize) -> Vec<u8> {
        for width in 1..=8u32 {
            let max = (1u128 << (7 * width)) - 1;
            if (value as u128) < max {
                let encoded = (1u128 << (7 * width)) | value as u128;
                return encoded.to_be_bytes()[(16 - width as usize)..].to_vec();
            }
        }
        panic!("the element is too large to encode");
    }

    /// A `Read + Seek` wrapper that counts the bytes actually read.
    struct CountingCursor {
        inner: Cursor<Vec<u8>>,
        read: std::cell::Cell<u64>,
    }

    impl Read for CountingCursor {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let count = self.inner.read(buf)?;
            self.read.set(self.read.get() + count as u64);
            Ok(count)
        }
    }

    impl Seek for CountingCursor {
        fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
            self.inner.seek(pos)
        }
    }

    #[test]
    fn skips_a_large_payload_without_reading_it() {
        let mut tracks = Vec::new();
        tracks.extend_from_slice(&element(ID_TRACK_ENTRY, &track_entry(1, 0)));
        let mut cluster = Vec::new();
        cluster.extend_from_slice(&element(ID_BLOCK_GROUP, &block_group(0x81, 0)));

        let mut segment = Vec::new();
        segment.extend_from_slice(&element(ID_TRACKS, &tracks));
        segment.extend_from_slice(&element(ID_CLUSTER, &cluster));
        // A 4 MiB Void element: the scanner must seek past it, not read it.
        let mut void = id(ID_VOID);
        void.extend_from_slice(&size_vint(4 * 1024 * 1024));
        void.extend_from_slice(&vec![0u8; 4 * 1024 * 1024]);
        segment.extend_from_slice(&void);

        let mut data = vec![0x1A, 0x45, 0xDF, 0xA3, 0x80];
        data.extend_from_slice(&id(ID_SEGMENT));
        data.extend_from_slice(&size_vint(segment.len()));
        data.extend_from_slice(&segment);

        let mut reader = CountingCursor {
            inner: Cursor::new(data.clone()),
            read: std::cell::Cell::new(0),
        };
        let trim = opus_trim(Path::new("memory.webm"), &mut reader, 1, 48_000)
            .expect("read the trim")
            .expect("some trim");
        assert_eq!(trim.blocks.len(), 1);
        assert!(
            reader.read.get() < 64 * 1024,
            "the reader read {} bytes of a {} byte file",
            reader.read.get(),
            data.len()
        );
    }

    #[test]
    fn rejects_a_cluster_that_runs_past_the_end() {
        let mut tracks = Vec::new();
        tracks.extend_from_slice(&element(ID_TRACK_ENTRY, &track_entry(1, 0)));

        // A cluster whose declared size is far larger than the bytes that follow.
        let mut cluster = id(ID_CLUSTER);
        cluster.extend_from_slice(&size_vint(10_000));
        cluster.extend_from_slice(&element(ID_SIMPLE_BLOCK, &[0x81, 0x00, 0x00, 0x80]));

        let mut segment = Vec::new();
        segment.extend_from_slice(&element(ID_TRACKS, &tracks));
        segment.extend_from_slice(&cluster);

        let mut data = vec![0x1A, 0x45, 0xDF, 0xA3, 0x80];
        data.extend_from_slice(&id(ID_SEGMENT));
        data.extend_from_slice(&size_vint(segment.len()));
        data.extend_from_slice(&segment);

        let error = read(&data, 1).expect_err("a truncated cluster must return an error");
        assert_eq!(error.kind(), DecodeErrorKind::Malformed);
    }
}
