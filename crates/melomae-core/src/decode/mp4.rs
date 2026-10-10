//! A minimal ISO base media file reader for the audio track's edit list.
//!
//! Symphonia exposes the container's `elst` atom but does not apply it, and its AAC decoder does
//! no gapless trim, so the encoder priming and padding would leak into the decoded output. The
//! edit list holds both: `media_time` is the media position the presentation starts at (the
//! priming), and the sum of the non-empty `segment_duration` values is how long the presentation
//! lasts (which drops the padding). `media_time` is in the media timescale; `segment_duration` is
//! in the movie timescale.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use super::{DecodeError, DecodeErrorKind};

/// The largest `moov` box this reader buffers. A larger one is metadata that cannot be trusted.
const MAX_MOOV_BYTES: u64 = 64 * 1024 * 1024;

/// The trim the audio track's edit list asks for, in samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EditListTrim {
    /// Frames to drop from the start of the decoded audio.
    pub start: usize,
    /// How many frames the presentation lasts, after the start trim.
    pub duration: usize,
}

/// A malformed `moov` or `elst` box: the reader cannot trust it.
fn malformed(path: &Path, reason: &str) -> DecodeError {
    DecodeError::new(DecodeErrorKind::Malformed, path, reason)
}

/// An edit list the reader understands, but whose sequence is outside the supported contract.
fn unsupported(path: &Path, reason: impl Into<String>) -> DecodeError {
    DecodeError::new(DecodeErrorKind::UnsupportedFormat, path, reason)
}

/// Read the edit list of the audio track whose `tkhd` track ID is `track_id`, or `Ok(None)` when
/// the file is not an ISO base media file, holds no such track, or has no edit list.
///
/// A malformed `moov` or `elst` is a `Malformed` error. An edit list whose sequence is not initial
/// empty edits followed by one contiguous unit-rate media range is an `UnsupportedFormat` error, so
/// the caller never silently falls back to an incorrect trim.
pub(crate) fn edit_list_trim(
    path: &Path,
    track_id: u32,
    sample_rate: u32,
) -> Result<Option<EditListTrim>, DecodeError> {
    let mut file =
        File::open(path).map_err(|e| DecodeError::new(DecodeErrorKind::Io, path, e.to_string()))?;
    edit_list_trim_reader(path, &mut file, track_id, sample_rate)
}

/// Read the audio track's edit list from an open reader, so a test can count the bytes read.
fn edit_list_trim_reader<R: Read + Seek>(
    path: &Path,
    reader: &mut R,
    track_id: u32,
    sample_rate: u32,
) -> Result<Option<EditListTrim>, DecodeError> {
    let Some(moov) = read_moov(reader) else {
        return Ok(None);
    };
    let movie_timescale = find_box(&moov, b"mvhd")
        .and_then(read_mvhd_timescale)
        .ok_or_else(|| malformed(path, "the movie header has no usable timescale"))?;

    for trak in find_boxes(&moov, b"trak") {
        // Select the reader's track by its `tkhd` identity, not by the first audio handler, so a
        // file whose selected track is not the first audio track uses the right edit list.
        let Some(tkhd) = find_box(trak, b"tkhd") else {
            continue;
        };
        if read_tkhd_track_id(tkhd) != Some(track_id) {
            continue;
        }
        let Some(mdia) = find_box(trak, b"mdia") else {
            continue;
        };
        let Some(hdlr) = find_box(mdia, b"hdlr") else {
            continue;
        };
        // The handler type sits after version/flags and the pre-defined field.
        if hdlr.get(8..12) != Some(b"soun") {
            continue;
        }
        let media_timescale = find_box(mdia, b"mdhd")
            .and_then(read_mdhd_timescale)
            .ok_or_else(|| malformed(path, "the media header has no usable timescale"))?;
        let Some(elst) = find_box(trak, b"edts").and_then(|edts| find_box(edts, b"elst")) else {
            return Ok(None);
        };
        let edits =
            read_edits(elst).ok_or_else(|| malformed(path, "the edit list is malformed"))?;
        return trim_from_edits(path, edits, media_timescale, movie_timescale, sample_rate);
    }
    Ok(None)
}

/// Turn a validated edit list into the trim the decoder applies.
///
/// Initial empty edits are skipped. Every media edit must use the unit rate and start where the
/// previous one ended, so the sequence is one contiguous media range. Anything else is an
/// `UnsupportedFormat` error: a fallback trim would decode the wrong audio.
fn trim_from_edits(
    path: &Path,
    edits: Vec<Edit>,
    media_timescale: u32,
    movie_timescale: u32,
    sample_rate: u32,
) -> Result<Option<EditListTrim>, DecodeError> {
    let mut media_time: Option<i64> = None;
    let mut next_media_time: Option<u64> = None;
    let mut seen_media = false;
    let mut movie_duration: u128 = 0;

    for edit in edits {
        if edit.media_time < 0 {
            if seen_media {
                return Err(unsupported(path, "an empty edit follows a media edit"));
            }
            continue;
        }
        seen_media = true;
        if !edit.is_unit_rate() {
            return Err(unsupported(
                path,
                format!(
                    "the edit list media rate {}.{} is not the unit rate",
                    edit.media_rate_integer, edit.media_rate_fraction
                ),
            ));
        }
        let start = u64::try_from(edit.media_time)
            .map_err(|_| unsupported(path, "an edit list media time is out of range"))?;
        if let Some(expected) = next_media_time {
            if start != expected {
                return Err(unsupported(
                    path,
                    "the edit list media ranges are not contiguous",
                ));
            }
        }
        // The media range runs for `segment_duration` movie ticks; convert to media samples.
        let range = u128::from(edit.segment_duration)
            .checked_mul(u128::from(media_timescale))
            .and_then(|scaled| scaled.checked_div(u128::from(movie_timescale)))
            .and_then(|samples| u64::try_from(samples).ok())
            .ok_or_else(|| unsupported(path, "an edit list duration is out of range"))?;
        next_media_time = Some(
            start
                .checked_add(range)
                .ok_or_else(|| unsupported(path, "an edit list duration is out of range"))?,
        );
        media_time.get_or_insert(edit.media_time);
        movie_duration = movie_duration
            .checked_add(u128::from(edit.segment_duration))
            .ok_or_else(|| unsupported(path, "an edit list duration is out of range"))?;
    }

    let Some(media_time) = media_time else {
        return Ok(None);
    };
    let media_time = u128::try_from(media_time)
        .map_err(|_| unsupported(path, "an edit list media time is out of range"))?;
    let start = media_time
        .checked_mul(u128::from(sample_rate))
        .and_then(|scaled| scaled.checked_div(u128::from(media_timescale)))
        .and_then(|samples| usize::try_from(samples).ok())
        .ok_or_else(|| unsupported(path, "an edit list start is out of range"))?;
    let duration = movie_duration
        .checked_mul(u128::from(sample_rate))
        .and_then(|scaled| scaled.checked_div(u128::from(movie_timescale)))
        .and_then(|samples| usize::try_from(samples).ok())
        .ok_or_else(|| unsupported(path, "an edit list duration is out of range"))?;
    Ok(Some(EditListTrim { start, duration }))
}

/// One edit list entry.
struct Edit {
    /// Duration in the movie timescale.
    segment_duration: u64,
    /// Media start time in the media timescale. `-1` marks an empty edit.
    media_time: i64,
    /// The integer part of `media_rate`, in the media timescale.
    media_rate_integer: i16,
    /// The fractional part of `media_rate`.
    media_rate_fraction: i16,
}

impl Edit {
    /// Whether the entry presents at the unit rate (1.0). A dwell or a rate change is not.
    fn is_unit_rate(&self) -> bool {
        self.media_rate_integer == 1 && self.media_rate_fraction == 0
    }
}

/// The `tkhd` track ID, so the reader can select the track the decoder chose.
fn read_tkhd_track_id(tkhd: &[u8]) -> Option<u32> {
    match *tkhd.first()? {
        0 => read_u32(tkhd, 12),
        1 => read_u32(tkhd, 20),
        _ => None,
    }
}

/// The bytes of the `moov` box, or `None` when the file is not an ISO base media file or has none.
///
/// Only the box headers are read to walk the top level, the media data is skipped by seeking, and
/// the `moov` payload is read into a buffer bounded by [`MAX_MOOV_BYTES`]. The reader therefore
/// never loads the whole file.
fn read_moov<R: Read + Seek>(reader: &mut R) -> Option<Vec<u8>> {
    let file_len = reader.seek(SeekFrom::End(0)).ok()?;
    let mut offset = 0u64;
    let mut is_iso = false;
    while offset.checked_add(8)? <= file_len {
        reader.seek(SeekFrom::Start(offset)).ok()?;
        let mut header = [0u8; 8];
        reader.read_exact(&mut header).ok()?;
        let mut size = u64::from(u32::from_be_bytes(header[..4].try_into().unwrap()));
        let kind: &[u8; 4] = header[4..8].try_into().unwrap();
        let mut header_len = 8u64;
        if size == 1 {
            let mut large = [0u8; 8];
            reader.read_exact(&mut large).ok()?;
            size = u64::from_be_bytes(large);
            header_len = 16;
        } else if size == 0 {
            size = file_len.checked_sub(offset)?;
        }
        if size < header_len {
            return None;
        }
        let body_start = offset.checked_add(header_len)?;
        let body_end = offset.checked_add(size)?;
        if body_end > file_len {
            return None;
        }
        if kind == b"ftyp" {
            is_iso = true;
        }
        if kind == b"moov" {
            if !is_iso {
                return None;
            }
            let body_len = body_end.checked_sub(body_start)?;
            if body_len > MAX_MOOV_BYTES {
                return None;
            }
            reader.seek(SeekFrom::Start(body_start)).ok()?;
            let mut body = vec![0u8; usize::try_from(body_len).ok()?];
            reader.read_exact(&mut body).ok()?;
            return Some(body);
        }
        offset = body_end;
    }
    None
}

/// The first child box of `kind` within `data`.
fn find_box<'a>(data: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    boxes(data)
        .find(|(name, _)| name.as_slice() == kind.as_slice())
        .map(|(_, body)| body)
}

/// Every child box of `kind` within `data`.
fn find_boxes<'a>(data: &'a [u8], kind: &'a [u8; 4]) -> impl Iterator<Item = &'a [u8]> {
    boxes(data)
        .filter(move |(name, _)| name.as_slice() == kind.as_slice())
        .map(|(_, body)| body)
}

/// Iterate the boxes in `data`, yielding the type and the payload.
fn boxes(data: &[u8]) -> impl Iterator<Item = (&[u8; 4], &[u8])> {
    let mut offset = 0usize;
    std::iter::from_fn(move || {
        let header = data.get(offset..offset + 8)?;
        let mut size = u32::from_be_bytes(header[..4].try_into().unwrap()) as u64;
        let kind: &[u8; 4] = header[4..8].try_into().unwrap();
        let mut header_len = 8usize;
        if size == 1 {
            let large = data.get(offset + 8..offset + 16)?;
            size = u64::from_be_bytes(large.try_into().unwrap());
            header_len = 16;
        }
        if size == 0 {
            size = (data.len() - offset) as u64;
        }
        if size < header_len as u64 {
            return None;
        }
        let body_start = offset.checked_add(header_len)?;
        let body_end = offset.checked_add(usize::try_from(size).ok()?)?;
        let body = data.get(body_start..body_end)?;
        offset = body_end;
        Some((kind, body))
    })
}

/// Read the timescale from an `mvhd` payload, in the movie timescale.
fn read_mvhd_timescale(mvhd: &[u8]) -> Option<u32> {
    let timescale = match *mvhd.first()? {
        0 => read_u32(mvhd, 12),
        1 => read_u32(mvhd, 20),
        _ => None,
    }?;
    // A zero timescale cannot be used to convert a duration to samples.
    (timescale != 0).then_some(timescale)
}

/// Read the timescale from an `mdhd` payload, in the media timescale.
fn read_mdhd_timescale(mdhd: &[u8]) -> Option<u32> {
    let timescale = match *mdhd.first()? {
        0 => read_u32(mdhd, 12),
        1 => read_u32(mdhd, 20),
        _ => None,
    }?;
    // A zero timescale cannot be used to convert a duration to samples.
    (timescale != 0).then_some(timescale)
}

/// Read a big-endian `u32` at `offset`.
fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        data.get(offset..offset + 4)?.try_into().unwrap(),
    ))
}

/// Read the entries of an `elst` payload.
fn read_edits(elst: &[u8]) -> Option<Vec<Edit>> {
    /// A count that large is malformed; do not allocate for it.
    const MAX_EDITS: u32 = 1024;

    let version = *elst.first()?;
    let count = read_u32(elst, 4)?;
    if count > MAX_EDITS {
        return None;
    }
    let entry_len = match version {
        0 => 12,
        1 => 20,
        _ => return None,
    };

    let mut edits = Vec::with_capacity(count as usize);
    for index in 0..count as usize {
        let start = 8usize.checked_add(index.checked_mul(entry_len)?)?;
        let entry = elst.get(start..start + entry_len)?;
        let (segment_duration, media_time, rate_integer, rate_fraction) = match version {
            0 => (
                u64::from(u32::from_be_bytes(entry[0..4].try_into().unwrap())),
                i64::from(i32::from_be_bytes(entry[4..8].try_into().unwrap())),
                i16::from_be_bytes(entry[8..10].try_into().unwrap()),
                i16::from_be_bytes(entry[10..12].try_into().unwrap()),
            ),
            _ => (
                u64::from_be_bytes(entry[0..8].try_into().unwrap()),
                i64::from_be_bytes(entry[8..16].try_into().unwrap()),
                i16::from_be_bytes(entry[16..18].try_into().unwrap()),
                i16::from_be_bytes(entry[18..20].try_into().unwrap()),
            ),
        };
        edits.push(Edit {
            segment_duration,
            media_time,
            media_rate_integer: rate_integer,
            media_rate_fraction: rate_fraction,
        });
    }
    Some(edits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn boxed(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + body.len());
        out.extend_from_slice(&((8 + body.len()) as u32).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(body);
        out
    }

    fn mvhd(timescale: u32) -> Vec<u8> {
        let mut body = vec![0u8; 20];
        body[12..16].copy_from_slice(&timescale.to_be_bytes());
        boxed(b"mvhd", &body)
    }

    fn mdhd(timescale: u32) -> Vec<u8> {
        let mut body = vec![0u8; 20];
        body[12..16].copy_from_slice(&timescale.to_be_bytes());
        boxed(b"mdhd", &body)
    }

    fn hdlr(handler: &[u8; 4]) -> Vec<u8> {
        let mut body = vec![0u8; 12];
        body[8..12].copy_from_slice(handler);
        boxed(b"hdlr", &body)
    }

    /// A version-0 `tkhd` whose track ID sits at offset 12.
    fn tkhd(track_id: u32) -> Vec<u8> {
        let mut body = vec![0u8; 16];
        body[12..16].copy_from_slice(&track_id.to_be_bytes());
        boxed(b"tkhd", &body)
    }

    /// An `elst` with version-0 entries of `(segment_duration, media_time)`.
    fn elst(entries: &[(u32, i32)]) -> Vec<u8> {
        let mut body = vec![0u8; 8];
        body[4..8].copy_from_slice(&(entries.len() as u32).to_be_bytes());
        for (duration, media_time) in entries {
            body.extend_from_slice(&duration.to_be_bytes());
            body.extend_from_slice(&media_time.to_be_bytes());
            body.extend_from_slice(&1u16.to_be_bytes());
            body.extend_from_slice(&0u16.to_be_bytes());
        }
        boxed(b"elst", &body)
    }

    fn file(movie_timescale: u32, media_timescale: u32, entries: &[(u32, i32)]) -> Vec<u8> {
        file_with_handler(b"soun", movie_timescale, media_timescale, entries)
    }

    fn file_with_handler(
        handler: &[u8; 4],
        movie_timescale: u32,
        media_timescale: u32,
        entries: &[(u32, i32)],
    ) -> Vec<u8> {
        file_with_tracks(movie_timescale, &[(1, *handler, media_timescale, entries)])
    }

    /// One audio track: its `tkhd` ID, its handler, its media timescale, and its edit entries.
    fn file_with_tracks(
        movie_timescale: u32,
        tracks: &[(u32, [u8; 4], u32, &[(u32, i32)])],
    ) -> Vec<u8> {
        let mut moov = Vec::new();
        moov.extend_from_slice(&mvhd(movie_timescale));
        for (track_id, handler, media_timescale, entries) in tracks {
            let mut mdia = Vec::new();
            mdia.extend_from_slice(&mdhd(*media_timescale));
            mdia.extend_from_slice(&hdlr(handler));

            let mut trak = Vec::new();
            trak.extend_from_slice(&tkhd(*track_id));
            trak.extend_from_slice(&boxed(b"mdia", &mdia));
            trak.extend_from_slice(&boxed(b"edts", &elst(entries)));
            moov.extend_from_slice(&boxed(b"trak", &trak));
        }

        let mut out = Vec::new();
        out.extend_from_slice(&boxed(b"ftyp", b"M4A "));
        out.extend_from_slice(&boxed(b"moov", &moov));
        out
    }

    fn write_temp(name: &str, data: &[u8]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join("melomae-decode-tests").join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, data).unwrap();
        path
    }

    /// An `elst` with version-0 entries of `(duration, media_time, rate_integer, rate_fraction)`.
    fn elst_with_rate(entries: &[(u32, i32, i16, i16)]) -> Vec<u8> {
        let mut body = vec![0u8; 8];
        body[4..8].copy_from_slice(&(entries.len() as u32).to_be_bytes());
        for (duration, media_time, rate_integer, rate_fraction) in entries {
            body.extend_from_slice(&duration.to_be_bytes());
            body.extend_from_slice(&media_time.to_be_bytes());
            body.extend_from_slice(&rate_integer.to_be_bytes());
            body.extend_from_slice(&rate_fraction.to_be_bytes());
        }
        boxed(b"elst", &body)
    }

    /// Read track 1's trim and require the expected value.
    fn assert_trim(path: &Path, expected: EditListTrim) {
        assert_eq!(edit_list_trim(path, 1, 44_100).unwrap(), Some(expected));
    }

    #[test]
    fn reads_the_start_and_the_presentation_duration() {
        // A 6 s presentation in a 1000-tick movie timescale, starting at media sample 1024.
        let path = write_temp("mp4_edit.m4a", &file(1_000, 44_100, &[(6_000, 1_024)]));
        assert_trim(
            &path,
            EditListTrim {
                start: 1_024,
                duration: 264_600,
            },
        );
    }

    #[test]
    fn converts_each_timescale_separately() {
        // Movie timescale 48_000 and media timescale 44_100: the two conversions differ.
        let path = write_temp(
            "mp4_timescales.m4a",
            &file(48_000, 44_100, &[(96_000, 2_048)]),
        );
        assert_trim(
            &path,
            EditListTrim {
                start: 2_048,
                duration: 88_200,
            },
        );
    }

    #[test]
    fn skips_an_initial_empty_edit() {
        let path = write_temp(
            "mp4_empty_edit.m4a",
            &file(1_000, 44_100, &[(500, -1), (6_000, 1_024)]),
        );
        assert_trim(
            &path,
            EditListTrim {
                start: 1_024,
                duration: 264_600,
            },
        );
    }

    #[test]
    fn sums_two_contiguous_edits() {
        // (3000, 1024) then (3000, 133324): the second media range starts where the first ends,
        // so the presentation is one contiguous six seconds.
        let path = write_temp(
            "mp4_two_edits.m4a",
            &file(1_000, 44_100, &[(3_000, 1_024), (3_000, 133_324)]),
        );
        assert_trim(
            &path,
            EditListTrim {
                start: 1_024,
                duration: 264_600,
            },
        );
    }

    #[test]
    fn rejects_repeated_media_ranges() {
        // Two identical ranges present the same three seconds twice, not six contiguous seconds.
        let path = write_temp(
            "mp4_repeated.m4a",
            &file(1_000, 44_100, &[(3_000, 1_024), (3_000, 1_024)]),
        );
        let error = edit_list_trim(&path, 1, 44_100).expect_err("repeated ranges must be rejected");
        assert_eq!(
            error.kind(),
            DecodeErrorKind::UnsupportedFormat,
            "reason: {}",
            error.reason()
        );
    }

    #[test]
    fn rejects_discontinuous_media_ranges() {
        let path = write_temp(
            "mp4_discontinuous.m4a",
            &file(1_000, 44_100, &[(3_000, 1_024), (3_000, 999_999)]),
        );
        let error = edit_list_trim(&path, 1, 44_100).expect_err("a gap must be rejected");
        assert_eq!(error.kind(), DecodeErrorKind::UnsupportedFormat);
    }

    #[test]
    fn rejects_a_non_unit_rate_edit() {
        // `media_rate` 0.0 is a dwell, not a presentation at the unit rate.
        let mut trak = Vec::new();
        trak.extend_from_slice(&tkhd(1));
        let mut mdia = Vec::new();
        mdia.extend_from_slice(&mdhd(44_100));
        mdia.extend_from_slice(&hdlr(b"soun"));
        trak.extend_from_slice(&boxed(b"mdia", &mdia));
        trak.extend_from_slice(&boxed(b"edts", &elst_with_rate(&[(6_000, 1_024, 0, 0)])));
        let mut moov = Vec::new();
        moov.extend_from_slice(&mvhd(1_000));
        moov.extend_from_slice(&boxed(b"trak", &trak));
        let mut out = Vec::new();
        out.extend_from_slice(&boxed(b"ftyp", b"M4A "));
        out.extend_from_slice(&boxed(b"moov", &moov));

        let path = write_temp("mp4_dwell.m4a", &out);
        let error = edit_list_trim(&path, 1, 44_100).expect_err("a dwell must be rejected");
        assert_eq!(error.kind(), DecodeErrorKind::UnsupportedFormat);
    }

    #[test]
    fn selects_the_edit_list_by_tkhd_track_id() {
        // Two audio tracks: track 1 trims to 1024, track 2 to 2048. Selecting a track must use its
        // own edit list, not the first audio track's.
        let data = file_with_tracks(
            1_000,
            &[
                (1, *b"soun", 44_100, &[(6_000, 1_024)]),
                (2, *b"soun", 44_100, &[(6_000, 2_048)]),
            ],
        );
        let path = write_temp("mp4_two_tracks.m4a", &data);
        assert_eq!(
            edit_list_trim(&path, 2, 44_100).unwrap(),
            Some(EditListTrim {
                start: 2_048,
                duration: 264_600
            })
        );
        assert_eq!(
            edit_list_trim(&path, 1, 44_100).unwrap(),
            Some(EditListTrim {
                start: 1_024,
                duration: 264_600
            })
        );
    }

    #[test]
    fn ignores_a_zero_trim() {
        let path = write_temp("mp4_zero_trim.m4a", &file(1_000, 44_100, &[(6_000, 0)]));
        assert_trim(
            &path,
            EditListTrim {
                start: 0,
                duration: 264_600,
            },
        );
    }

    #[test]
    fn reads_version_one_entries() {
        let mut body = vec![0u8; 8];
        body[0] = 1;
        body[4..8].copy_from_slice(&1u32.to_be_bytes());
        body.extend_from_slice(&6_000u64.to_be_bytes());
        body.extend_from_slice(&1_024i64.to_be_bytes());
        body.extend_from_slice(&1u16.to_be_bytes());
        body.extend_from_slice(&0u16.to_be_bytes());

        let mut trak = Vec::new();
        let mut mdia = Vec::new();
        mdia.extend_from_slice(&mdhd(44_100));
        mdia.extend_from_slice(&hdlr(b"soun"));
        trak.extend_from_slice(&tkhd(1));
        trak.extend_from_slice(&boxed(b"mdia", &mdia));
        trak.extend_from_slice(&boxed(b"edts", &boxed(b"elst", &body)));
        let mut moov = Vec::new();
        moov.extend_from_slice(&mvhd(1_000));
        moov.extend_from_slice(&boxed(b"trak", &trak));
        let mut out = Vec::new();
        out.extend_from_slice(&boxed(b"ftyp", b"M4A "));
        out.extend_from_slice(&boxed(b"moov", &moov));

        let path = write_temp("mp4_v1.m4a", &out);
        assert_trim(
            &path,
            EditListTrim {
                start: 1_024,
                duration: 264_600,
            },
        );
    }

    #[test]
    fn ignores_a_video_track_edit_list() {
        let path = write_temp(
            "mp4_video.m4a",
            &file_with_handler(b"vide", 1_000, 44_100, &[(6_000, 1_024)]),
        );
        assert_eq!(edit_list_trim(&path, 1, 44_100).unwrap(), None);
    }

    #[test]
    fn ignores_a_non_mp4_file() {
        let path = write_temp(
            "mp4_not_a_container.bin",
            b"not an ISO base media file at all",
        );
        assert_eq!(edit_list_trim(&path, 1, 44_100).unwrap(), None);
    }

    #[test]
    fn rejects_a_zero_media_timescale() {
        let path = write_temp("mp4_zero_timescale.m4a", &file(1_000, 0, &[(6_000, 1_024)]));
        let error =
            edit_list_trim(&path, 1, 44_100).expect_err("a zero media timescale is malformed");
        assert_eq!(error.kind(), DecodeErrorKind::Malformed);
    }

    #[test]
    fn rejects_a_truncated_entry_table() {
        // The count claims two entries but the payload holds one.
        let mut elst = elst(&[(6_000, 1_024)]);
        elst[12..16].copy_from_slice(&2u32.to_be_bytes());

        let mut trak = Vec::new();
        let mut mdia = Vec::new();
        mdia.extend_from_slice(&mdhd(44_100));
        mdia.extend_from_slice(&hdlr(b"soun"));
        trak.extend_from_slice(&tkhd(1));
        trak.extend_from_slice(&boxed(b"mdia", &mdia));
        trak.extend_from_slice(&boxed(b"edts", &elst));
        let mut moov = Vec::new();
        moov.extend_from_slice(&mvhd(1_000));
        moov.extend_from_slice(&boxed(b"trak", &trak));
        let mut out = Vec::new();
        out.extend_from_slice(&boxed(b"ftyp", b"M4A "));
        out.extend_from_slice(&boxed(b"moov", &moov));

        let path = write_temp("mp4_truncated.m4a", &out);
        let error = edit_list_trim(&path, 1, 44_100).expect_err("a truncated table is malformed");
        assert_eq!(error.kind(), DecodeErrorKind::Malformed);
    }

    #[test]
    fn rejects_a_zero_movie_timescale() {
        let path = write_temp(
            "mp4_zero_movie_timescale.m4a",
            &file(0, 44_100, &[(6_000, 1_024)]),
        );
        let error =
            edit_list_trim(&path, 1, 44_100).expect_err("a zero movie timescale is malformed");
        assert_eq!(error.kind(), DecodeErrorKind::Malformed);
    }

    #[test]
    fn rejects_an_oversized_extended_box() {
        // `ftyp`, then a box whose extended 64-bit size is `u64::MAX`. The reader must not
        // allocate it, and must not overflow on `offset + size`.
        let mut out = boxed(b"ftyp", b"M4A ");
        out.extend_from_slice(&1u32.to_be_bytes());
        out.extend_from_slice(b"free");
        out.extend_from_slice(&u64::MAX.to_be_bytes());
        let path = write_temp("mp4_oversized_box.m4a", &out);
        assert_eq!(edit_list_trim(&path, 1, 44_100).unwrap(), None);
    }

    /// A `Read + Seek` wrapper that counts the bytes actually read.
    struct CountingReader {
        inner: fs::File,
        read: std::cell::Cell<u64>,
    }

    impl std::io::Read for CountingReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let count = self.inner.read(buf)?;
            self.read.set(self.read.get() + count as u64);
            Ok(count)
        }
    }

    impl std::io::Seek for CountingReader {
        fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
            self.inner.seek(pos)
        }
    }

    #[test]
    fn reads_only_the_boxes_it_needs() {
        // A small `moov` followed by a 4 MiB `free` box. The reader must stop at the `moov`.
        let mut data = file(1_000, 44_100, &[(6_000, 1_024)]);
        data.extend_from_slice(&boxed(b"free", &vec![0u8; 4 * 1024 * 1024]));
        let path = write_temp("mp4_large_payload.m4a", &data);

        let mut reader = CountingReader {
            inner: fs::File::open(&path).unwrap(),
            read: std::cell::Cell::new(0),
        };
        assert!(
            edit_list_trim_reader(Path::new("mp4_large_payload.m4a"), &mut reader, 1, 44_100)
                .unwrap()
                .is_some()
        );
        assert!(
            reader.read.get() < 64 * 1024,
            "the reader read {} bytes of a {} byte file",
            reader.read.get(),
            data.len()
        );
    }
}
