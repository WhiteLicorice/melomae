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
use std::path::{Path, PathBuf};

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

    /// Every channel, in order.
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

    let format = symphonia::default::get_probe()
        .probe(
            &hint,
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| DecodeError::new(DecodeErrorKind::UnsupportedFormat, path, e.to_string()))?;

    decode_format(path, format)
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

    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(SymphoniaError::ResetRequired) => break,
            // An unknown-size Segment or Cluster runs to the end of the file; that clean end
            // surfaces as an unexpected EOF from the demuxer, not a truncation.
            Err(SymphoniaError::IoError(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof =>
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
        mp4::edit_list_trim(path, rate)
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
    let opus_channels = match channel_count {
        1 => OpusChannels::Mono,
        2 => OpusChannels::Stereo,
        other => {
            return Err(DecodeError::new(
                DecodeErrorKind::UnsupportedFormat,
                path,
                format!("Opus supports one or two channels, not {other}"),
            ))
        }
    };
    // Matroska carries the delay as the selected track's `CodecDelay` and the padding as each of
    // its blocks' `DiscardPadding`; symphonia surfaces neither. The scanner reads only those
    // elements through its own bounded reader, so it never loads the whole file. Ogg is not EBML,
    // so the scanner returns `None` and the Ogg path below runs instead.
    let matroska_trim = {
        let mut reader = File::open(path)
            .map_err(|e| DecodeError::new(DecodeErrorKind::Io, path, e.to_string()))?;
        matroska::opus_trim(path, &mut reader, u64::from(track_id), OPUS_SAMPLE_RATE)?
    };

    let pre_skip = match &matroska_trim {
        Some(trim) => trim.pre_skip,
        None => opus_pre_skip(extra_data).ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::UnsupportedFormat,
                path,
                "the Opus track has no pre-skip",
            )
        })?,
    };
    let mut decoder = OpusDecoder::new(opus_channels, OpusSampleRate::Hz48000)
        .map_err(|e| DecodeError::new(DecodeErrorKind::Decode, path, e.message()))?;

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
            // end as an unexpected EOF from its EBML iterator, which is not a truncation.
            Err(SymphoniaError::IoError(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof =>
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
            .decode_float_to_slice(&packet.data, &mut block[..], false)
            .map_err(|e| DecodeError::new(DecodeErrorKind::Decode, path, e.message()))?;

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
                apply_block_trim(path, &mut channels, block_start, blocks[block_index].padding)?;
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

/// Read the pre-skip from the `OpusHead` identification header in a track's extra data.
fn opus_pre_skip(extra_data: Option<&[u8]>) -> Option<usize> {
    let data = extra_data?;
    if data.len() < 12 || &data[..8] != b"OpusHead" {
        return None;
    }
    Some(usize::from(u16::from_le_bytes([data[10], data[11]])))
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
}
