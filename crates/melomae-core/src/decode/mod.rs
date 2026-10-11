//! Decode every supported input format to planar 32-bit float audio.
//!
//! [`decode_file`] demuxes and decodes MP3, AAC/M4A, Opus (Ogg and WebM), Vorbis, FLAC, ALAC,
//! WAV, and AIFF, then resamples to [`TARGET_SAMPLE_RATE`]. Opus packets are decoded with
//! libopus, the reference decoder; every other codec is decoded with Symphonia. The channel
//! count of the input is preserved.

mod matroska;
mod mp4;
mod resample;

use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use opusic_c::multistream::{Config as OpusMsConfig, Decoder as OpusMsDecoder};
use opusic_c::{Channels as OpusChannels, Decoder as OpusDecoder, SampleRate as OpusSampleRate};
use symphonia::core::codecs::audio::well_known::CODEC_ID_OPUS;
use symphonia::core::codecs::audio::{AudioCodecParameters, AudioDecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::TimeBase;

/// The sample rate Apollo expects. [`decode_file`] resamples every input to this rate.
pub const TARGET_SAMPLE_RATE: u32 = 44_100;

/// The version of the linked libopus, for diagnostics and for recording against a reference.
pub fn opus_libopus_version() -> &'static str {
    opusic_c::version()
}

/// The rate libopus always decodes at, whatever rate the container signals.
const OPUS_SAMPLE_RATE: u32 = 48_000;

/// The longest Opus packet is 120 ms, so at most this many frames per channel come back.
const OPUS_MAX_FRAMES_PER_PACKET: usize = (OPUS_SAMPLE_RATE as usize * 120) / 1000;

/// The class of a [`DecodeError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeErrorKind {
    /// The file could not be opened or read.
    Io,
    /// The container or its codec is not supported, or the container is malformed.
    UnsupportedFormat,
    /// The container is malformed: an element is truncated or internally inconsistent.
    Malformed,
    /// The format is supported, but a packet could not be decoded.
    Decode,
    /// The container holds no decodable audio track.
    NoAudioTrack,
}

/// An error from decoding an input file.
///
/// The error always names the file and states the reason, so a caller can report both.
#[derive(Debug)]
pub struct DecodeError {
    kind: DecodeErrorKind,
    path: PathBuf,
    reason: String,
}

impl DecodeError {
    fn new(kind: DecodeErrorKind, path: &Path, reason: impl Into<String>) -> Self {
        Self {
            kind,
            path: path.to_path_buf(),
            reason: reason.into(),
        }
    }

    /// The class of failure.
    pub fn kind(&self) -> DecodeErrorKind {
        self.kind
    }

    /// The file that could not be decoded.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Why the file could not be decoded.
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.reason)
    }
}

impl std::error::Error for DecodeError {}

/// Audio as planar 32-bit float channels.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioBuffer {
    sample_rate: u32,
    channels: Vec<Vec<f32>>,
}

impl AudioBuffer {
    /// Wrap planar channels at a sample rate.
    ///
    /// # Panics
    ///
    /// Panics if `channels` is empty or the planes do not all have the same length.
    pub fn new(sample_rate: u32, channels: Vec<Vec<f32>>) -> Self {
        assert!(!channels.is_empty(), "audio must have at least one channel");
        let frames = channels[0].len();
        assert!(
            channels.iter().all(|channel| channel.len() == frames),
            "audio planes must all be the same length"
        );
        Self {
            sample_rate,
            channels,
        }
    }

    /// The sample rate in Hertz.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// The number of channels.
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    /// The number of frames in each channel.
    pub fn frames(&self) -> usize {
        self.channels[0].len()
    }

    /// Every channel, in the codec's native order.
    ///
    /// For Opus this is the Vorbis channel order defined by RFC 7845 section 5.1.1.2. The order is
    /// not normalized across codecs, and no speaker layout is retained: the channel count is the
    /// only layout information.
    pub fn channels(&self) -> &[Vec<f32>] {
        &self.channels
    }

    /// One channel, if the index is in range.
    pub fn channel(&self, index: usize) -> Option<&[f32]> {
        self.channels.get(index).map(Vec::as_slice)
    }

    /// Take the planar channels.
    pub fn into_channels(self) -> Vec<Vec<f32>> {
        self.channels
    }

    /// Resample to `target_sample_rate`.
    ///
    /// A buffer that is already at the target rate, or that holds no frames, is copied unchanged.
    pub fn resample_to(&self, target_sample_rate: u32) -> AudioBuffer {
        if self.sample_rate == target_sample_rate || self.frames() == 0 {
            return AudioBuffer {
                sample_rate: target_sample_rate,
                channels: self.channels.clone(),
            };
        }
        AudioBuffer::new(
            target_sample_rate,
            resample::planar(&self.channels, self.sample_rate, target_sample_rate),
        )
    }
}

/// Decode a file and resample it to [`TARGET_SAMPLE_RATE`].
pub fn decode_file(path: impl AsRef<Path>) -> Result<AudioBuffer, DecodeError> {
    let native = decode_file_native(path.as_ref())?;
    Ok(native.resample_to(TARGET_SAMPLE_RATE))
}

/// Decode a file, keeping its native sample rate.
///
/// Opus always decodes at 48 kHz. Every other codec keeps the rate the container signals.
pub fn decode_file_native(path: &Path) -> Result<AudioBuffer, DecodeError> {
    let file =
        File::open(path).map_err(|e| DecodeError::new(DecodeErrorKind::Io, path, e.to_string()))?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(extension);
    }

    let format = match symphonia::default::get_probe().probe(
        &hint,
        stream,
        FormatOptions::default(),
        MetadataOptions::default(),
    ) {
        Ok(format) => format,
        Err(error) => return Err(probe_failure(path, error)),
    };

    decode_format(path, format)
}

/// The error for a failed probe. An Ogg Opus stream whose header the container probe rejects still
/// names itself when its mapping family is out of scope: read the `OpusHead` from the first Ogg
/// page directly, so a family-255 file reports the family rather than the dependency's rejection.
fn probe_failure(path: &Path, error: SymphoniaError) -> DecodeError {
    let named = read_ogg_opus_head(path)
        .and_then(|head| parse_opus_head(path, Some(&head)).err())
        .filter(|header| header.kind() == DecodeErrorKind::UnsupportedFormat);
    match named {
        Some(header) => DecodeError::new(
            DecodeErrorKind::UnsupportedFormat,
            path,
            header.reason().to_string(),
        ),
        None => DecodeError::new(DecodeErrorKind::UnsupportedFormat, path, error.to_string()),
    }
}

/// The Opus identification packet from the first Ogg page, when the file starts with one.
///
/// The `OpusHead` is the first packet of the first page, so the page header and its segment table
/// locate it. Only the first page is read, into a fixed-size buffer.
fn read_ogg_opus_head(path: &Path) -> Option<Vec<u8>> {
    let mut file = File::open(path).ok()?;
    let mut buffer = [0u8; 64 * 1024];
    let read = file.read(&mut buffer).ok()?;
    let data = &buffer[..read];
    if data.len() < 27 || &data[..4] != b"OggS" {
        return None;
    }
    let table_end = 27usize.checked_add(usize::from(data[26]))?;
    if data.len() < table_end {
        return None;
    }
    let mut packet_len = 0usize;
    for &lacing in &data[27..table_end] {
        packet_len = packet_len.checked_add(usize::from(lacing))?;
        if lacing < 255 {
            break;
        }
    }
    let packet_end = table_end.checked_add(packet_len)?;
    if packet_len < 8 || data.len() < packet_end {
        return None;
    }
    Some(data[table_end..packet_end].to_vec())
}

fn decode_format(
    path: &Path,
    mut format: Box<dyn FormatReader>,
) -> Result<AudioBuffer, DecodeError> {
    let (track_id, codec_params, track_time_base, track_duration) = {
        let track = format.default_track(TrackType::Audio).ok_or_else(|| {
            DecodeError::new(DecodeErrorKind::NoAudioTrack, path, "no audio track")
        })?;
        let codec_params = track.codec_params.clone().ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::NoAudioTrack,
                path,
                "the audio track has no codec parameters",
            )
        })?;
        (track.id, codec_params, track.time_base, track.duration)
    };

    let audio = codec_params.audio().ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::NoAudioTrack,
            path,
            "the track is not audio",
        )
    })?;
    let channel_count = audio
        .channels
        .as_ref()
        .map_or(0, |channels| channels.count());
    if channel_count == 0 {
        return Err(DecodeError::new(
            DecodeErrorKind::UnsupportedFormat,
            path,
            "the audio track has no channel information",
        ));
    }

    if audio.codec == CODEC_ID_OPUS {
        return decode_opus(
            path,
            &mut *format,
            track_id,
            channel_count,
            audio.extra_data.as_deref(),
        );
    }

    let sample_rate = audio.sample_rate.ok_or_else(|| {
        DecodeError::new(
            DecodeErrorKind::UnsupportedFormat,
            path,
            "unknown sample rate",
        )
    })?;
    // The track's duration is the number of valid media frames, in timebase units. When the
    // timebase is the reciprocal of the sample rate, that is a frame count, and the difference
    // from the decoded total is the container's trailing encoder padding.
    let valid_frames = if track_time_base == TimeBase::try_from_recip(sample_rate) {
        track_duration.and_then(|duration| usize::try_from(duration.get()).ok())
    } else {
        None
    };
    decode_with_symphonia(
        path,
        &mut *format,
        track_id,
        sample_rate,
        audio,
        valid_frames,
    )
}

/// Only an ISO base media file carries an edit list the decoder must apply. Every other container
/// (WAV, FLAC, MP3, Vorbis, AIFF) must not even open the edit-list reader.
fn needs_edit_list(short_name: &str) -> bool {
    short_name == "isomp4"
}

fn decode_with_symphonia(
    path: &Path,
    format: &mut dyn FormatReader,
    track_id: u32,
    sample_rate: u32,
    params: &AudioCodecParameters,
    valid_frames: Option<usize>,
) -> Result<AudioBuffer, DecodeError> {
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())
        .map_err(|e| DecodeError::new(DecodeErrorKind::UnsupportedFormat, path, e.to_string()))?;

    let mut channels: Vec<Vec<f32>> = Vec::new();
    let mut planes: Vec<Vec<f32>> = Vec::new();
    let mut rate = sample_rate;

    // An unknown-size Matroska Segment runs to the end of the file, which the demuxer reports as
    // an unexpected EOF. A structural scan proves that end is valid. Every other format, and a
    // truncated Matroska file, treats the same error as truncation.
    let accept_eof_at_end = if format.format_info().short_name == "mkv" {
        let mut reader = File::open(path)
            .map_err(|e| DecodeError::new(DecodeErrorKind::Io, path, e.to_string()))?;
        matroska::opus_trim(path, &mut reader, 0, sample_rate)?.valid_eof
    } else {
        false
    };

    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(SymphoniaError::ResetRequired) => break,
            Err(SymphoniaError::IoError(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof && accept_eof_at_end =>
            {
                break
            }
            Err(e) => {
                return Err(DecodeError::new(
                    DecodeErrorKind::Decode,
                    path,
                    e.to_string(),
                ))
            }
        };
        if packet.track_id != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(buffer) => {
                rate = buffer.spec().rate();
                buffer.copy_to_vecs_planar::<f32>(&mut planes);
                if channels.is_empty() {
                    channels = vec![Vec::new(); planes.len()];
                }
                for (channel, plane) in channels.iter_mut().zip(&planes) {
                    channel.extend_from_slice(plane);
                }
            }
            // A single bad packet is skipped; the rest of the file still decodes.
            Err(SymphoniaError::DecodeError(_)) | Err(SymphoniaError::IoError(_)) => continue,
            Err(SymphoniaError::ResetRequired) => break,
            Err(e) => {
                return Err(DecodeError::new(
                    DecodeErrorKind::Decode,
                    path,
                    e.to_string(),
                ))
            }
        }
    }

    if channels.is_empty() {
        return Err(DecodeError::new(
            DecodeErrorKind::Decode,
            path,
            "no decodable audio frames",
        ));
    }

    // Match ffmpeg's gapless length. The edit list gives the start trim and the presentation
    // length, so the end is exactly `start + duration`. Formats without an edit list fall back to
    // the track's declared duration, and otherwise keep every decoded frame.
    let decoded_frames = channels[0].len();
    // Only an MP4 container has an edit list. Skip the reader for every other format, so a WAV or
    // FLAC decode never opens it.
    let edit = if needs_edit_list(format.format_info().short_name) {
        mp4::edit_list_trim(path, track_id, rate)?
    } else {
        None
    };
    let start_trim = edit.map_or(0, |edit| edit.start);
    let valid_frames = match edit {
        Some(edit) => Some(start_trim.saturating_add(edit.duration)),
        None => valid_frames,
    };
    let end = valid_frames.unwrap_or(decoded_frames).min(decoded_frames);
    let start_trim = start_trim.min(end);
    for channel in &mut channels {
        channel.truncate(end);
        channel.drain(..start_trim);
    }
    Ok(AudioBuffer::new(rate, channels))
}

fn decode_opus(
    path: &Path,
    format: &mut dyn FormatReader,
    track_id: u32,
    channel_count: usize,
    extra_data: Option<&[u8]>,
) -> Result<AudioBuffer, DecodeError> {
    let head = parse_opus_head(path, extra_data)?;
    if head.channels != channel_count {
        return Err(DecodeError::new(
            DecodeErrorKind::Malformed,
            path,
            format!(
                "the Opus header declares {} channels but the track declares {channel_count}",
                head.channels
            ),
        ));
    }
    // Matroska carries the delay as the selected track's `CodecDelay` and the padding as each of
    // its blocks' `DiscardPadding`; symphonia surfaces neither. The scanner reads only those
    // elements through its own bounded reader, so it never loads the whole file. Ogg is not EBML,
    // so the scanner returns `None` and the Ogg path below runs instead.
    let matroska = {
        let mut reader = File::open(path)
            .map_err(|e| DecodeError::new(DecodeErrorKind::Io, path, e.to_string()))?;
        matroska::opus_trim(path, &mut reader, u64::from(track_id), OPUS_SAMPLE_RATE)?
    };
    // A clean end of file is only a valid boundary for a Matroska stream whose structural scan
    // proved it. A truncated WebM fails the scan above, before this loop runs.
    let accept_eof_at_end = matroska.valid_eof;
    let matroska_trim = matroska.trim;

    let pre_skip = match &matroska_trim {
        Some(trim) => trim.pre_skip,
        None => head.pre_skip,
    };
    let mut decoder = build_opus_backend(path, &head)?;

    let mut channels: Vec<Vec<f32>> = vec![Vec::new(); channel_count];
    let mut block = vec![0f32; OPUS_MAX_FRAMES_PER_PACKET * channel_count];
    let mut trailing_padding = 0usize;

    // Each Matroska block holds `frames` decoded packets; apply its trim when the block ends.
    let blocks: &[matroska::BlockTrim] = matroska_trim.as_ref().map_or(&[], |trim| &trim.blocks);
    let mut block_index = 0usize;
    let mut packets_left = blocks.first().map_or(0, |entry| entry.frames);
    let mut block_start = 0usize;
    let mut in_block = !blocks.is_empty();

    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(SymphoniaError::ResetRequired) => break,
            // An unknown-size Segment runs to the end of the file. Symphonia reports that clean
            // end as an unexpected EOF from its EBML iterator. Accept it only when the structural
            // scan validated the end; otherwise it is a truncation.
            Err(SymphoniaError::IoError(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof && accept_eof_at_end =>
            {
                break
            }
            Err(e) => {
                return Err(DecodeError::new(
                    DecodeErrorKind::Decode,
                    path,
                    e.to_string(),
                ))
            }
        };
        if packet.track_id != track_id {
            continue;
        }

        let decoded = decoder
            .decode(&packet.data, &mut block[..])
            .map_err(|reason| DecodeError::new(DecodeErrorKind::Decode, path, reason))?;

        if matroska_trim.is_none() {
            trailing_padding += packet.trim_end.get() as usize;
        }
        for (channel, plane) in channels.iter_mut().enumerate() {
            plane.reserve(decoded);
            for frame in 0..decoded {
                plane.push(block[frame * channel_count + channel]);
            }
        }

        if in_block {
            packets_left = packets_left.saturating_sub(1);
            if packets_left == 0 {
                apply_block_trim(
                    path,
                    &mut channels,
                    block_start,
                    blocks[block_index].padding,
                )?;
                block_index += 1;
                block_start = channels.first().map_or(0, Vec::len);
                match blocks.get(block_index) {
                    Some(entry) => packets_left = entry.frames,
                    None => in_block = false,
                }
            }
        }
    }

    let total = channels.first().map_or(0, Vec::len);
    if total == 0 {
        return Err(DecodeError::new(
            DecodeErrorKind::Decode,
            path,
            "no decodable Opus frames",
        ));
    }

    // libopus returns the whole decoded block, including the encoder's pre-skip. Symphonia does
    // not turn the Opus pre-skip into a per-packet trim, so drop it from the start here, using
    // the value from the identification header. For Ogg the trailing padding comes from the last
    // packets' trim; for Matroska the block trims above already removed both ends. Together these
    // make the decoded length exact.
    let end = total
        .checked_sub(trailing_padding)
        .filter(|end| *end >= pre_skip)
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::Decode,
                path,
                "the Opus pre-skip and padding exceed the decoded frame count",
            )
        })?;
    for plane in &mut channels {
        plane.truncate(end);
        plane.drain(..pre_skip);
    }

    // The `OpusHead` output gain is a signed Q7.8 decibel value. Apply it once as a float scale,
    // in both the single-stream and the multistream path; neither binding applies it for us. Keep
    // the samples as float and add no clipping.
    if head.output_gain != 0 {
        let scale = 10f32.powf(f32::from(head.output_gain) / (20.0 * 256.0));
        for plane in &mut channels {
            for sample in plane.iter_mut() {
                *sample *= scale;
            }
        }
    }
    Ok(AudioBuffer::new(OPUS_SAMPLE_RATE, channels))
}

/// Remove one Matroska block's `DiscardPadding` from the decoded audio: a positive value from the
/// end of the block, a negative value from its start. The block runs from `start` to the current
/// end of `channels`, which is where every block so far has been appended.
fn apply_block_trim(
    path: &Path,
    channels: &mut [Vec<f32>],
    start: usize,
    padding: i64,
) -> Result<(), DecodeError> {
    let end = channels.first().map_or(0, Vec::len);
    let decoded = end.saturating_sub(start);
    let drop = usize::try_from(padding.unsigned_abs()).unwrap_or(usize::MAX);
    if drop == 0 {
        return Ok(());
    }
    if drop > decoded {
        return Err(DecodeError::new(
            DecodeErrorKind::Malformed,
            path,
            "a DiscardPadding is larger than its decoded block",
        ));
    }
    if padding > 0 {
        for channel in channels.iter_mut() {
            channel.truncate(end - drop);
        }
    } else {
        for channel in channels.iter_mut() {
            channel.drain(start..start + drop);
        }
    }
    Ok(())
}

/// The `OpusHead` identification header fields the decoder needs.
///
/// RFC 7845 section 5.1 defines the layout: bytes 0 to 7 are the magic, byte 8 is the version,
/// byte 9 is the channel count, bytes 10 and 11 are the pre-skip, bytes 16 and 17 are the signed
/// output gain in Q7.8 decibels, byte 18 is the channel mapping family, byte 19 is the stream
/// count, byte 20 is the coupled stream count, and bytes 21 onward are the mapping table, one
/// entry per channel.
#[derive(Debug)]
struct OpusHead {
    /// The channel count, from byte 9.
    channels: usize,
    /// The pre-skip, in samples at 48 kHz, from bytes 10 and 11.
    pre_skip: usize,
    /// The output gain, signed Q7.8 decibels, from bytes 16 and 17.
    output_gain: i16,
    /// The channel mapping family, from byte 18.
    mapping_family: u8,
    /// The number of elementary streams, from byte 19 (mapping family 0 always has one).
    stream_count: u8,
    /// The number of coupled streams, from byte 20.
    coupled_count: u8,
    /// The mapping table, one entry per channel.
    mapping: Vec<u8>,
}

/// Parse and validate the `OpusHead` identification header. A malformed or unsupported header
/// returns a typed error that names the file and the offending field.
fn parse_opus_head(path: &Path, extra_data: Option<&[u8]>) -> Result<OpusHead, DecodeError> {
    let malformed = |reason: &str| DecodeError::new(DecodeErrorKind::Malformed, path, reason);
    let data =
        extra_data.ok_or_else(|| malformed("the Opus track has no identification header"))?;
    if data.len() < 19 || &data[..8] != b"OpusHead" {
        return Err(malformed(
            "the Opus identification header is missing or truncated",
        ));
    }
    // RFC 7845 section 5.1: a version is compatible when its upper four bits are zero. Anything
    // else is a different, incompatible format.
    if data[8] >> 4 != 0 {
        return Err(malformed("the Opus header version is incompatible"));
    }
    let channels = usize::from(data[9]);
    if channels == 0 {
        return Err(malformed("the Opus header declares no channels"));
    }
    let pre_skip = usize::from(u16::from_le_bytes([data[10], data[11]]));
    let output_gain = i16::from_le_bytes([data[16], data[17]]);
    let mapping_family = data[18];
    let (stream_count, coupled_count, mapping) = match mapping_family {
        0 => {
            if channels > 2 {
                return Err(malformed(
                    "the Opus header declares more than two channels in mapping family 0",
                ));
            }
            (1, u8::from(channels == 2), (0..channels as u8).collect())
        }
        1 => {
            let table_end = 21usize
                .checked_add(channels)
                .ok_or_else(|| malformed("the Opus header channel count is out of range"))?;
            if data.len() < table_end {
                return Err(malformed(
                    "the Opus identification header is missing its mapping table",
                ));
            }
            let stream_count = data[19];
            let coupled_count = data[20];
            if stream_count == 0 || coupled_count > stream_count {
                return Err(malformed(
                    "the Opus header declares an invalid stream count",
                ));
            }
            let total = usize::from(stream_count) + usize::from(coupled_count);
            // RFC 7845 section 5.1 allows a decoded channel to be unassigned, so the stream and
            // coupled counts may exceed the channel count. The mapping table indexes into N + M
            // decoded streams, or 255 for silence.
            if total > 255 {
                return Err(malformed("the Opus header declares more than 255 streams"));
            }
            let mapping = data[21..table_end].to_vec();
            if mapping
                .iter()
                .any(|value| *value != 255 && usize::from(*value) >= total)
            {
                return Err(malformed(
                    "the Opus header's channel mapping is out of range",
                ));
            }
            (stream_count, coupled_count, mapping)
        }
        // Ruling for task 03: mapping family 255 is out of scope in every container. It must name
        // itself and its channel count, never the dependency's container-level rejection.
        255 => {
            return Err(DecodeError::new(
                DecodeErrorKind::UnsupportedFormat,
                path,
                format!("the Opus mapping family 255 with {channels} channels is not supported"),
            ))
        }
        other => {
            return Err(DecodeError::new(
                DecodeErrorKind::UnsupportedFormat,
                path,
                format!("the Opus channel mapping family {other} is not supported"),
            ))
        }
    };
    Ok(OpusHead {
        channels,
        pre_skip,
        output_gain,
        mapping_family,
        stream_count,
        coupled_count,
        mapping,
    })
}

/// The libopus decoder for one file: the single-stream decoder for unpacked mono or stereo, and
/// the multistream decoder for a mapped channel layout.
enum OpusBackend {
    Single(OpusDecoder),
    Mapped(OpusMsDecoder),
}

impl OpusBackend {
    /// Decode one packet into `output`, returning the number of frames per channel.
    fn decode(&mut self, packet: &[u8], output: &mut [f32]) -> Result<usize, String> {
        match self {
            Self::Single(decoder) => decoder
                .decode_float_to_slice(packet, output, false)
                .map_err(|e| e.message().to_string()),
            Self::Mapped(decoder) => decoder
                .decode_float_to_slice(packet, output, false)
                .map_err(|e| e.message().to_string()),
        }
    }
}

/// Build the decoder for a validated `OpusHead`.
///
/// Mapping family 0 is unpacked mono or stereo and uses the single-stream decoder. Every other
/// family carries a mapping table and uses libopus's multistream decoder, whose channel count is a
/// compile-time constant, so each supported count gets its own arm.
fn build_opus_backend(path: &Path, head: &OpusHead) -> Result<OpusBackend, DecodeError> {
    let rate = OpusSampleRate::Hz48000;
    if head.mapping_family == 0 {
        let opus_channels = match head.channels {
            1 => OpusChannels::Mono,
            2 => OpusChannels::Stereo,
            _ => {
                return Err(DecodeError::new(
                    DecodeErrorKind::Malformed,
                    path,
                    "the Opus header declares more than two channels in mapping family 0",
                ))
            }
        };
        let decoder = OpusDecoder::new(opus_channels, rate)
            .map_err(|e| DecodeError::new(DecodeErrorKind::Decode, path, e.message()))?;
        return Ok(OpusBackend::Single(decoder));
    }

    // Mapping family 1 uses the multistream decoder, whose channel count is a compile-time constant
    // of at most eight. A legal header outside that scope is unsupported, not malformed. Mapping
    // family 255 is rejected at the header, before this is reached.
    let total = usize::from(head.stream_count) + usize::from(head.coupled_count);
    if head.channels > 8 || total > head.channels {
        return Err(DecodeError::new(
            DecodeErrorKind::UnsupportedFormat,
            path,
            format!(
                "the Opus mapping family {} with {} channels and {} + {} streams is outside the \
                 supported range: family 0 unpacked mono and stereo, and family 1 up to eight \
                 channels whose stream and coupled counts sum to no more than the channel count",
                head.mapping_family, head.channels, head.stream_count, head.coupled_count
            ),
        ));
    }

    macro_rules! mapped {
        ($n:literal) => {{
            let mut mapping = [0u8; $n];
            mapping.copy_from_slice(&head.mapping[..$n]);
            let config =
                OpusMsConfig::<$n>::try_new(head.stream_count, head.coupled_count, mapping)
                    .ok_or_else(|| {
                        DecodeError::new(
                            DecodeErrorKind::Malformed,
                            path,
                            "the Opus channel mapping is invalid",
                        )
                    })?;
            let decoder = OpusMsDecoder::new(config, rate)
                .map_err(|e| DecodeError::new(DecodeErrorKind::Decode, path, e.message()))?;
            Ok(OpusBackend::Mapped(decoder))
        }};
    }

    match head.channels {
        1 => mapped!(1),
        2 => mapped!(2),
        3 => mapped!(3),
        4 => mapped!(4),
        5 => mapped!(5),
        6 => mapped!(6),
        7 => mapped!(7),
        8 => mapped!(8),
        other => Err(DecodeError::new(
            DecodeErrorKind::UnsupportedFormat,
            path,
            format!("Opus decode supports at most eight channels, not {other}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_to_the_same_rate_is_a_copy() {
        let buffer = AudioBuffer::new(44_100, vec![vec![0.0, 0.25, -0.5], vec![0.5, 0.0, 0.25]]);
        let resampled = buffer.resample_to(44_100);
        assert_eq!(resampled, buffer);
    }

    #[test]
    fn decode_error_display_names_the_file_and_the_reason() {
        let error = DecodeError::new(
            DecodeErrorKind::Decode,
            Path::new("song.opus"),
            "bad packet",
        );
        assert_eq!(error.to_string(), "song.opus: bad packet");
        assert_eq!(error.reason(), "bad packet");
        assert_eq!(error.kind(), DecodeErrorKind::Decode);
    }

    #[test]
    fn decode_reports_a_missing_file_by_name() {
        let error = decode_file(Path::new("does-not-exist.mp3")).expect_err("must fail");
        assert_eq!(error.kind(), DecodeErrorKind::Io);
        assert!(error.to_string().contains("does-not-exist.mp3"));
    }

    #[test]
    #[should_panic(expected = "same length")]
    fn audio_buffer_rejects_ragged_planes() {
        AudioBuffer::new(44_100, vec![vec![0.0, 0.5], vec![0.0]]);
    }

    #[test]
    fn decode_source_never_spawns_a_process() {
        // The decoder must not depend on ffmpeg: only the test setup runs it. The needles are
        // built at runtime so this test does not match its own source.
        let needles = [
            concat!("Command", "::new"),
            concat!("process", "::Command"),
            concat!("std", "::process"),
        ];
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("decode");
        for entry in std::fs::read_dir(&dir).expect("read the decode source directory") {
            let path = entry.expect("read a decode source entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("read a decode source file");
            for needle in needles {
                assert!(
                    !text.contains(needle),
                    "{} must not spawn a process ({needle})",
                    path.display()
                );
            }
        }
    }

    #[test]
    fn only_the_mp4_container_needs_the_edit_list() {
        // These are symphonia's container short names. Only the MP4 reader carries an edit list.
        for short_name in ["wav", "flac", "mp3", "ogg", "aiff", "mkv"] {
            assert!(
                !needs_edit_list(short_name),
                "{short_name} must not open the edit-list reader"
            );
        }
        assert!(needs_edit_list("isomp4"));
    }

    /// A minimal `OpusHead` with the given channel count, mapping family, stream counts, and
    /// mapping table.
    fn opus_head(
        channels: u8,
        mapping_family: u8,
        stream_count: u8,
        coupled_count: u8,
        mapping: &[u8],
    ) -> Vec<u8> {
        let mut head = vec![0u8; 21 + usize::from(channels)];
        head[..8].copy_from_slice(b"OpusHead");
        head[9] = channels;
        head[18] = mapping_family;
        if mapping_family != 0 {
            head[19] = stream_count;
            head[20] = coupled_count;
            head[21..21 + mapping.len()].copy_from_slice(mapping);
        }
        head
    }

    #[test]
    fn a_bad_opus_head_returns_a_typed_error() {
        // Mapping family 0 allows only one or two channels, so a six-channel family 0 header is
        // malformed and must be rejected before any decoder is built.
        let head = opus_head(6, 0, 0, 0, &[]);
        let error = parse_opus_head(Path::new("bad.opus"), Some(&head)).expect_err("a bad header");
        assert_eq!(error.kind(), DecodeErrorKind::Malformed);
        assert!(
            error.reason().contains("mapping family 0"),
            "the error must name the bad field, got: {}",
            error.reason()
        );

        // A truncated header has no complete mapping table and is also malformed.
        let mut truncated = opus_head(6, 1, 4, 2, &[0, 1, 2, 3, 4, 5]);
        truncated.truncate(20);
        let error =
            parse_opus_head(Path::new("bad.opus"), Some(&truncated)).expect_err("truncated");
        assert_eq!(error.kind(), DecodeErrorKind::Malformed);

        // A coupled count larger than the stream count is malformed.
        let head = opus_head(6, 1, 2, 3, &[0, 1, 2, 3]);
        let error =
            parse_opus_head(Path::new("bad.opus"), Some(&head)).expect_err("bad stream counts");
        assert_eq!(error.kind(), DecodeErrorKind::Malformed);
    }

    #[test]
    fn a_valid_opus_head_reports_its_mapping() {
        // The standard 5.1 layout: four streams, two of them coupled, in Vorbis channel order.
        let head = opus_head(6, 1, 4, 2, &[0, 1, 2, 3, 4, 5]);
        let parsed = parse_opus_head(Path::new("good.opus"), Some(&head)).expect("a valid header");
        assert_eq!(parsed.channels, 6);
        assert_eq!(parsed.mapping_family, 1);
        assert_eq!(parsed.stream_count, 4);
        assert_eq!(parsed.coupled_count, 2);
        assert_eq!(parsed.mapping, vec![0, 1, 2, 3, 4, 5]);

        let stereo = parse_opus_head(Path::new("good.opus"), Some(&opus_head(2, 0, 0, 0, &[])))
            .expect("a valid stereo header");
        assert_eq!(stereo.stream_count, 1);
        assert_eq!(stereo.coupled_count, 1);
        assert_eq!(stereo.mapping, vec![0, 1]);
    }

    /// B3: a legal header with an unassigned decoded channel (C=2, N=2, M=1) parses, but the
    /// backend is out of scope and must return a typed `UnsupportedFormat`, never `Malformed`.
    #[test]
    fn an_unassigned_decoded_channel_is_unsupported() {
        let head = opus_head(2, 1, 2, 1, &[0, 1]);
        let parsed =
            parse_opus_head(Path::new("unused.opus"), Some(&head)).expect("a legal header");
        let error = match build_opus_backend(Path::new("unused.opus"), &parsed) {
            Ok(_) => panic!("an unassigned decoded channel is out of scope"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), DecodeErrorKind::UnsupportedFormat);
        let reason = error.reason().to_string();
        assert!(reason.contains("family 1"), "{reason}");
        assert!(reason.contains("2 channels"), "{reason}");
        assert!(reason.contains("2") && reason.contains("1"), "{reason}");
    }

    /// D4: mapping family 255 is out of scope in every container, so the header parse itself
    /// returns a typed `UnsupportedFormat` that names the family and the channel count. This
    /// replaces the round-2 tests that expected family 255 within eight channels to build.
    #[test]
    fn family_255_is_unsupported_and_names_itself() {
        let head = opus_head(6, 255, 4, 2, &[0, 1, 2, 3, 4, 5]);
        let error = parse_opus_head(Path::new("f255.opus"), Some(&head))
            .expect_err("family 255 is out of scope");
        assert_eq!(error.kind(), DecodeErrorKind::UnsupportedFormat);
        let reason = error.reason().to_string();
        assert!(reason.contains("mapping family 255"), "{reason}");
        assert!(reason.contains("6 channels"), "{reason}");
    }

    /// B3: a version byte whose upper four bits are not zero is incompatible and malformed.
    #[test]
    fn an_incompatible_opus_version_is_malformed() {
        let mut head = opus_head(2, 0, 0, 0, &[]);
        head[8] = 16;
        let error = parse_opus_head(Path::new("bad.opus"), Some(&head))
            .expect_err("an incompatible version");
        assert_eq!(error.kind(), DecodeErrorKind::Malformed);
    }
}
