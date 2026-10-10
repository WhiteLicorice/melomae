//! A minimal ISO base media file reader for the audio track's edit-list start trim.
//!
//! Symphonia exposes the container's `elst` atom but does not apply it, and its AAC decoder does
//! no gapless trim, so the encoder priming would leak into the decoded output. ffmpeg applies the
//! edit list's `media_time` as a start trim and nothing at the end; this module reads the same
//! value so the decoded length matches.

use std::fs;
use std::path::Path;

/// The number of leading frames the audio track's edit list says to skip, in samples.
///
/// Returns `None` when the file is not an ISO base media file, or when it has no audio edit list.
pub(crate) fn edit_list_start_trim(path: &Path, sample_rate: u32) -> Option<usize> {
    let data = fs::read(path).ok()?;
    find_box(&data, b"ftyp")?;
    let moov = find_box(&data, b"moov")?;

    for trak in find_boxes(moov, b"trak") {
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
        let timescale = find_box(mdia, b"mdhd").and_then(read_mdhd_timescale)?;
        let elst = find_box(trak, b"edts").and_then(|edts| find_box(edts, b"elst"))?;
        let media_time = read_first_media_time(elst)?;
        if media_time <= 0 {
            return Some(0);
        }
        // `media_time` is in the media timescale, which for audio is the sample rate.
        let samples =
            (u128::from(media_time as u64) * u128::from(sample_rate)) / u128::from(timescale);
        return Some(samples as usize);
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
        let body_start = offset + header_len;
        let body_end = offset + size as usize;
        let body = data.get(body_start..body_end)?;
        offset = body_end;
        Some((kind, body))
    })
}

/// Read the timescale from an `mdhd` payload.
fn read_mdhd_timescale(mdhd: &[u8]) -> Option<u32> {
    let version = *mdhd.first()?;
    match version {
        0 => Some(u32::from_be_bytes(mdhd.get(12..16)?.try_into().unwrap())),
        1 => Some(u32::from_be_bytes(mdhd.get(20..24)?.try_into().unwrap())),
        _ => None,
    }
}

/// Read the `media_time` of the first `elst` entry.
fn read_first_media_time(elst: &[u8]) -> Option<i64> {
    let version = *elst.first()?;
    let count = u32::from_be_bytes(elst.get(4..8)?.try_into().unwrap());
    if count == 0 {
        return None;
    }
    match version {
        0 => Some(i64::from(i32::from_be_bytes(
            elst.get(12..16)?.try_into().unwrap(),
        ))),
        1 => Some(i64::from_be_bytes(elst.get(16..24)?.try_into().unwrap())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boxed(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + body.len());
        out.extend_from_slice(&((8 + body.len()) as u32).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(body);
        out
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

    fn elst(media_time: i32) -> Vec<u8> {
        let mut body = vec![0u8; 16];
        body[4..8].copy_from_slice(&1u32.to_be_bytes());
        body[8..12].copy_from_slice(&6_000u32.to_be_bytes());
        body[12..16].copy_from_slice(&media_time.to_be_bytes());
        boxed(b"elst", &body)
    }

    fn file(handler: &[u8; 4], media_time: i32) -> Vec<u8> {
        let mut trak = Vec::new();
        let mut mdia = Vec::new();
        mdia.extend_from_slice(&mdhd(44_100));
        mdia.extend_from_slice(&hdlr(handler));
        trak.extend_from_slice(&boxed(b"mdia", &mdia));
        trak.extend_from_slice(&boxed(b"edts", &elst(media_time)));

        let mut moov = Vec::new();
        moov.extend_from_slice(&boxed(b"trak", &trak));
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

    #[test]
    fn reads_the_audio_edit_list_start_trim() {
        let path = write_temp("mp4_edit_1024.m4a", &file(b"soun", 1024));
        assert_eq!(edit_list_start_trim(&path, 44_100), Some(1024));
    }

    #[test]
    fn ignores_a_zero_edit_list_start_trim() {
        let path = write_temp("mp4_edit_zero.m4a", &file(b"soun", 0));
        assert_eq!(edit_list_start_trim(&path, 44_100), Some(0));
    }

    #[test]
    fn ignores_a_video_track_edit_list() {
        let path = write_temp("mp4_video_edit.m4a", &file(b"vide", 1024));
        assert_eq!(edit_list_start_trim(&path, 44_100), None);
    }

    #[test]
    fn ignores_a_non_mp4_file() {
        let path = write_temp(
            "mp4_not_a_container.bin",
            b"not an ISO base media file at all",
        );
        assert_eq!(edit_list_start_trim(&path, 44_100), None);
    }
}
