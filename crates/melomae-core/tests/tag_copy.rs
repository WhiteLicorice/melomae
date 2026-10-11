//! Copy every tag and picture from a source file into each output format.
//!
//! The three source fixtures are built in `setup::tag_fixtures` with the pinned ffmpeg and lofty.
//! Each source copies to FLAC, WAV, and Opus. Every source field and the front cover must survive,
//! or appear in the returned list of fields the target cannot hold.
//!
//! The expected source manifest is checked against a native reader before the copier runs, so a
//! field the fixture never wrote, or that lofty's generic layer loses, is caught. Each output is
//! then checked against the same manifest and the literal target name.

mod common;
mod setup;

use std::path::Path;

use lofty::config::WriteOptions;
use lofty::id3::v2::{Frame, FrameId, Id3v2Tag, TextInformationFrame};
use lofty::picture::PictureType;
use lofty::prelude::TagExt;
use lofty::tag::TagType;
use lofty::TextEncoding;

use melomae_core::decode::decode_file;
use melomae_core::encode::{encode_with_tags, OutputFormat};
use melomae_core::tag_copy::{read_tags, TagSet};

use setup::tag_fixtures::{
    expected_manifest, native_keys, native_manifest, source_kind, NativeKind, Source, TagFixtures,
    ALBUM_ARTIST, ALBUM_ARTIST_SORT, COMPOSERS, CUSTOM_NAME, CUSTOM_VALUE, ORIGINAL_DATE,
    ORIGINAL_YEAR, RELEASE_STATUS, RELEASE_STATUS_NATIVE, SCRIPT, TITLE,
};

/// The output formats, with the native tag kind and a file-name extension.
const OUTPUTS: [(OutputFormat, NativeKind, &str); 3] = [
    (OutputFormat::Flac, NativeKind::Flac, "flac"),
    (OutputFormat::Wav, NativeKind::Wav, "wav"),
    (OutputFormat::Opus, NativeKind::Opus, "opus"),
];

/// Sort `(name, value)` pairs, so two manifests compare as multisets.
fn sorted(mut pairs: Vec<(String, String)>) -> Vec<(String, String)> {
    pairs.sort();
    pairs
}

/// The expected manifest of a source as owned pairs.
fn manifest(source: Source) -> Vec<(String, String)> {
    expected_manifest(source)
        .into_iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect()
}

/// The portable pairs the production reader returns for a file: a standard field by its Vorbis
/// key, a custom field by its own name. Distinct names stay distinct, so LABEL and PUBLISHER never
/// collapse into one.
fn read_portable(tags: &TagSet) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for item in tags.standard() {
        let name = item
            .key()
            .map_key(TagType::VorbisComments)
            .map(str::to_string)
            .unwrap_or_else(|| format!("{:?}", item.key()));
        let value = item
            .value()
            .text()
            .or_else(|| item.value().locator())
            .unwrap_or_default()
            .to_string();
        pairs.push((name, value));
    }
    for field in tags.custom() {
        for value in field.values() {
            pairs.push((field.name().to_string(), value.clone()));
        }
    }
    pairs
}

/// Finding 4: the expected source manifest must match a native reader before the copier runs. The
/// native reader is format-specific, so it catches a field the fixture never wrote, or that the
/// generic tag layer loses.
#[test]
fn the_native_source_matches_the_expected_manifest() {
    let fixtures = TagFixtures::build();

    for (source, family) in fixtures.sources() {
        let path = fixtures.path(source);
        let native = native_manifest(&path, source_kind(source));
        assert_eq!(
            sorted(native),
            sorted(manifest(source)),
            "{family}: the native reader and the expected manifest must agree"
        );

        // The generic reader must see the same fields, so the copier cannot start from a partial set.
        let tags = read_tags(&path).expect("read the source tags");
        let seen = read_portable(&tags);
        for (name, value) in manifest(source) {
            assert!(
                seen.contains(&(name.clone(), value.clone())),
                "{family}: read_tags did not see {name}={value:?}"
            );
        }

        // The original year is a Vorbis field only, and the script must survive in every source.
        let has_year = native_manifest(&path, source_kind(source))
            .iter()
            .any(|(name, _)| name == "ORIGINALYEAR");
        assert_eq!(
            has_year,
            source == Source::Flac,
            "{family}: the original year is recorded for a Vorbis source only"
        );
        assert!(
            native_keys(&path, source_kind(source))
                .iter()
                .any(|key| key.ends_with("SCRIPT")),
            "{family}: the fixture must write the script natively"
        );
    }
}

/// Test-first plan item 2: for each source family, every field of the manifest and the front cover
/// survives in all three output formats, or the field is named in the list the target cannot hold.
#[test]
fn e2e_tag_copy_carries_every_field_into_every_output() {
    let fixtures = TagFixtures::build();
    let outputs = setup::TempDir::new("melomae-tag-copy");

    for (source, family) in fixtures.sources() {
        let path = fixtures.path(source);
        let tags = read_tags(&path).unwrap_or_else(|error| panic!("read {family}: {error}"));
        assert_eq!(
            tags.pictures().len(),
            1,
            "{family}: the fixture holds one cover"
        );
        let audio = decode_file(&path).unwrap_or_else(|error| panic!("decode {family}: {error}"));

        for (format, kind, extension) in OUTPUTS {
            let target = outputs.join(&format!("{family}.{extension}"));
            let (_, unsupported) = encode_with_tags(&audio, &target, format, Some(&tags))
                .unwrap_or_else(|error| panic!("{family} -> {extension}: {error}"));

            // The tagged output must still decode.
            decode_file(&target)
                .unwrap_or_else(|error| panic!("decode {family}.{extension}: {error}"));

            let written = native_manifest(&target, kind);
            for (name, value) in manifest(source) {
                let present = written.contains(&(name.clone(), value.clone()));
                assert!(
                    present || unsupported.contains(&name),
                    "{family} -> {extension}: {name}={value:?} was dropped without a report"
                );
            }

            let cover = read_tags(&target)
                .unwrap_or_else(|error| panic!("read {family}.{extension}: {error}"));
            assert_eq!(
                cover.pictures()[0].data(),
                tags.pictures()[0].data(),
                "{family} -> {extension}: the cover bytes must survive"
            );
            assert_eq!(
                cover.pictures()[0].mime_type(),
                Some(&lofty::picture::MimeType::Jpeg),
                "{family} -> {extension}: the cover MIME type must survive"
            );
        }
    }
}

/// Scope item 3 and 6: a standard field maps to the target format's own name, and a front cover
/// stays a front cover with its description, in the formats that can hold a type.
#[test]
fn e2e_tag_copy_maps_standard_fields_and_keeps_the_front_cover() {
    let fixtures = TagFixtures::build();
    let outputs = setup::TempDir::new("melomae-tag-mapping");

    for (source, family) in fixtures.sources() {
        let path = fixtures.path(source);
        let tags = read_tags(&path).expect("read the source tags");
        let audio = decode_file(&path).expect("decode the source");

        for (format, kind, extension) in OUTPUTS {
            let target = outputs.join(&format!("{family}-mapped.{extension}"));
            let (_, _) = encode_with_tags(&audio, &target, format, Some(&tags)).expect("encode");

            let written = native_manifest(&target, kind);
            assert!(
                written.contains(&("ALBUMARTIST".to_string(), ALBUM_ARTIST.to_string())),
                "{family} -> {extension}: the album artist did not survive as ALBUMARTIST"
            );
            assert!(
                written.contains(&("ALBUMARTISTSORT".to_string(), ALBUM_ARTIST_SORT.to_string())),
                "{family} -> {extension}: the album artist sort order did not survive"
            );
            assert!(
                written.contains(&(CUSTOM_NAME.to_string(), CUSTOM_VALUE.to_string())),
                "{family} -> {extension}: the custom field did not survive"
            );
            for composer in COMPOSERS {
                assert!(
                    written.contains(&("COMPOSER".to_string(), composer.to_string())),
                    "{family} -> {extension}: composer {composer:?} was lost, kept {written:?}"
                );
            }

            let output = read_tags(&target).expect("read the output tags");
            if source == Source::M4a {
                // MP4 holds no picture type, so its front cover reads back as a plain picture.
                assert_eq!(
                    output.pictures()[0].pic_type(),
                    PictureType::Other,
                    "{family} -> {extension}: MP4 cannot store a picture type"
                );
            } else {
                assert_eq!(
                    output.pictures()[0].pic_type(),
                    PictureType::CoverFront,
                    "{family} -> {extension}: a front cover must stay a front cover"
                );
                assert_eq!(
                    output.pictures()[0].description(),
                    Some(setup::tag_fixtures::COVER_DESCRIPTION),
                    "{family} -> {extension}: the cover description must survive"
                );
            }
        }
    }
}

/// Finding 1: an ID3 `TPUB` frame maps to the Vorbis `LABEL` field, not to `PUBLISHER`.
#[test]
fn picard_label_maps_id3_tpub_into_vorbis() {
    let fixtures = TagFixtures::build();
    let outputs = setup::TempDir::new("melomae-tag-label");
    let path = fixtures.path(Source::Mp3);

    assert!(
        native_keys(&path, NativeKind::Mp3)
            .iter()
            .any(|key| key == "TPUB"),
        "the MP3 fixture must hold a native TPUB frame"
    );

    let tags = read_tags(&path).expect("read the source tags");
    let audio = decode_file(&path).expect("decode the source");

    for (format, kind, extension) in [
        (OutputFormat::Flac, NativeKind::Flac, "flac"),
        (OutputFormat::Opus, NativeKind::Opus, "opus"),
    ] {
        let target = outputs.join(&format!("label.{extension}"));
        encode_with_tags(&audio, &target, format, Some(&tags)).expect("encode");
        let keys = native_keys(&target, kind);
        assert!(
            keys.iter().any(|key| key == "LABEL"),
            "{extension}: TPUB must map to the Vorbis LABEL key, got {keys:?}"
        );
        assert!(
            !keys.iter().any(|key| key == "PUBLISHER"),
            "{extension}: TPUB must not become PUBLISHER, got {keys:?}"
        );
    }

    // And back: the Vorbis LABEL field writes the ID3 TPUB frame.
    let target = outputs.join("label.wav");
    encode_with_tags(&audio, &target, OutputFormat::Wav, Some(&tags)).expect("encode WAV");
    assert!(
        native_keys(&target, NativeKind::Wav)
            .iter()
            .any(|key| key == "TPUB"),
        "LABEL must map back to the ID3 TPUB frame"
    );
}

/// Finding 1: the release status maps to Vorbis `RELEASESTATUS` from the ID3 and MP4 forms Picard
/// documents, and from Vorbis into the ID3 form.
#[test]
fn picard_release_status_maps_across_the_formats() {
    let fixtures = TagFixtures::build();
    let outputs = setup::TempDir::new("melomae-tag-status");

    for (source, native_name) in [(Source::Mp3, "MP3"), (Source::M4a, "MP4")] {
        let path = fixtures.path(source);
        let tags = read_tags(&path).expect("read the source tags");
        assert!(
            read_portable(&tags)
                .contains(&("RELEASESTATUS".to_string(), RELEASE_STATUS.to_string())),
            "{native_name}: the release status must read as RELEASESTATUS"
        );
        let audio = decode_file(&path).expect("decode the source");

        for (format, kind, extension) in [
            (OutputFormat::Flac, NativeKind::Flac, "flac"),
            (OutputFormat::Opus, NativeKind::Opus, "opus"),
        ] {
            let target = outputs.join(&format!("{native_name}.{extension}"));
            encode_with_tags(&audio, &target, format, Some(&tags)).expect("encode");
            assert!(
                native_manifest(&target, kind)
                    .contains(&("RELEASESTATUS".to_string(), RELEASE_STATUS.to_string())),
                "{native_name} -> {extension}: the release status must become RELEASESTATUS"
            );
        }
    }

    // A Vorbis release status writes the Picard `TXXX` frame in a WAV output. Picard writes the
    // description with its spaces, so the frame id is `TXXX:MusicBrainz Album Status`.
    let path = fixtures.path(Source::Flac);
    let tags = read_tags(&path).expect("read the source tags");
    let audio = decode_file(&path).expect("decode the source");
    let target = outputs.join("status.wav");
    encode_with_tags(&audio, &target, OutputFormat::Wav, Some(&tags)).expect("encode WAV");
    let expected = format!("TXXX:{RELEASE_STATUS_NATIVE}");
    assert!(
        native_keys(&target, NativeKind::Wav).contains(&expected),
        "a Vorbis RELEASESTATUS must write the ID3 {expected} frame"
    );
}

/// Finding 1: the script fits a WAV output as the `TXXX:SCRIPT` frame Picard documents, and is not
/// reported as a field the target cannot hold.
#[test]
fn picard_script_fits_a_wav_txxx_frame() {
    let fixtures = TagFixtures::build();
    let outputs = setup::TempDir::new("melomae-tag-script");

    for (source, family) in fixtures.sources() {
        let path = fixtures.path(source);
        let tags = read_tags(&path).expect("read the source tags");
        let audio = decode_file(&path).expect("decode the source");

        let target = outputs.join(&format!("{family}-script.wav"));
        let (_, unsupported) =
            encode_with_tags(&audio, &target, OutputFormat::Wav, Some(&tags)).expect("encode WAV");
        assert!(
            !unsupported.contains(&"SCRIPT".to_string()),
            "{family}: the script fits a WAV TXXX frame, got {unsupported:?}"
        );
        assert!(
            native_keys(&target, NativeKind::Wav)
                .iter()
                .any(|key| key == "TXXX:SCRIPT"),
            "{family}: the script must write the TXXX:SCRIPT frame"
        );
        assert!(
            native_manifest(&target, NativeKind::Wav)
                .contains(&("SCRIPT".to_string(), SCRIPT.to_string())),
            "{family}: the script value must survive in the WAV output"
        );
    }
}

/// Finding 2: the original year and the original date stay separate. Both keep their own native
/// name in FLAC and Opus. A WAV output has no Picard field for the original year, so it is
/// reported, and the original date stays unchanged.
#[test]
fn original_year_and_date_stay_separate() {
    let fixtures = TagFixtures::build();
    let outputs = setup::TempDir::new("melomae-tag-originalyear");
    let path = fixtures.path(Source::Flac);

    let tags = read_tags(&path).expect("read the source tags");
    let seen = read_portable(&tags);
    assert!(
        seen.contains(&("ORIGINALDATE".to_string(), ORIGINAL_DATE.to_string())),
        "the source must hold the original date"
    );
    assert!(
        seen.contains(&("ORIGINALYEAR".to_string(), ORIGINAL_YEAR.to_string())),
        "the source must hold the original year as its own field"
    );
    let audio = decode_file(&path).expect("decode the source");

    for (format, kind, extension) in [
        (OutputFormat::Flac, NativeKind::Flac, "flac"),
        (OutputFormat::Opus, NativeKind::Opus, "opus"),
    ] {
        let target = outputs.join(&format!("original.{extension}"));
        encode_with_tags(&audio, &target, format, Some(&tags)).expect("encode");
        let written = native_manifest(&target, kind);
        assert!(
            written.contains(&("ORIGINALDATE".to_string(), ORIGINAL_DATE.to_string())),
            "{extension}: the original date must stay ORIGINALDATE"
        );
        assert!(
            written.contains(&("ORIGINALYEAR".to_string(), ORIGINAL_YEAR.to_string())),
            "{extension}: the original year must stay ORIGINALYEAR, got {written:?}"
        );
    }

    let target = outputs.join("original.wav");
    let (_, unsupported) =
        encode_with_tags(&audio, &target, OutputFormat::Wav, Some(&tags)).expect("encode WAV");
    assert!(
        unsupported.contains(&"ORIGINALYEAR".to_string()),
        "the WAV output must report the original year, got {unsupported:?}"
    );
    assert!(
        native_manifest(&target, NativeKind::Wav)
            .contains(&("ORIGINALDATE".to_string(), ORIGINAL_DATE.to_string())),
        "the original date must survive the WAV output"
    );
}

/// Scope item 7: a field the target cannot hold is named in the returned list.
#[test]
fn e2e_tag_copy_reports_a_field_the_target_cannot_hold() {
    let fixtures = TagFixtures::build();
    let outputs = setup::TempDir::new("melomae-tag-unsupported");
    let path = fixtures.path(Source::Flac);
    let tags = read_tags(&path).expect("read the source tags");
    let audio = decode_file(&path).expect("decode the source");

    // ID3v2 has no frame for the original year, so a WAV output reports it.
    let target = outputs.join("unsupported.wav");
    let (_, unsupported) =
        encode_with_tags(&audio, &target, OutputFormat::Wav, Some(&tags)).expect("encode WAV");
    assert!(
        unsupported.contains(&"ORIGINALYEAR".to_string()),
        "the original year must be reported for WAV, got {unsupported:?}"
    );
    assert!(
        !native_manifest(&target, NativeKind::Wav)
            .iter()
            .any(|(name, _)| name == "ORIGINALYEAR"),
        "a field the target cannot hold must not appear"
    );

    // The same field survives in FLAC and Opus, which do hold it.
    for (format, kind, extension) in [
        (OutputFormat::Flac, NativeKind::Flac, "flac"),
        (OutputFormat::Opus, NativeKind::Opus, "opus"),
    ] {
        let target = outputs.join(&format!("supported.{extension}"));
        let (_, unsupported) =
            encode_with_tags(&audio, &target, format, Some(&tags)).expect("encode");
        assert!(
            !unsupported.contains(&"ORIGINALYEAR".to_string()),
            "{extension}: the original year is supported"
        );
        assert!(
            native_manifest(&target, kind)
                .contains(&("ORIGINALYEAR".to_string(), ORIGINAL_YEAR.to_string())),
            "{extension}: the original year must survive"
        );
    }
}

/// Finding 3: ID3v2.4 stores several artist values as one frame with null separators. Both values
/// must survive.
#[test]
fn id3v24_null_separated_artist_stays_two_values() {
    let fixtures = TagFixtures::build();
    let path = fixtures.path(Source::Mp3);
    write_id3(&path, |tag| {
        tag.insert(text_frame("TPE1", "Artist One\0Artist Two"));
        tag.insert(text_frame("TCOM", "Composer One\0Composer Two"));
    });

    let tags = read_tags(&path).expect("read the source tags");
    let artists: Vec<&str> = tags
        .standard()
        .iter()
        .filter(|item| item.key() == lofty::tag::ItemKey::TrackArtist)
        .filter_map(|item| item.value().text())
        .collect();
    assert_eq!(
        artists,
        vec!["Artist One", "Artist Two"],
        "a null-separated v2.4 artist must keep two values"
    );
    let composers: Vec<&str> = tags
        .standard()
        .iter()
        .filter(|item| item.key() == lofty::tag::ItemKey::Composer)
        .filter_map(|item| item.value().text())
        .collect();
    assert_eq!(
        composers,
        vec!["Composer One", "Composer Two"],
        "a null-separated v2.4 composer must keep two values"
    );
}

/// Finding 3: a single ID3v2.4 value that contains a slash stays one value. A writer must not split
/// on the slash.
#[test]
fn id3v24_single_value_with_a_slash_stays_one_value() {
    let fixtures = TagFixtures::build();
    let path = fixtures.path(Source::Mp3);
    write_id3(&path, |tag| {
        tag.insert(text_frame("TPE1", "AC/DC"));
    });

    let tags = read_tags(&path).expect("read the source tags");
    let artists: Vec<&str> = tags
        .standard()
        .iter()
        .filter(|item| item.key() == lofty::tag::ItemKey::TrackArtist)
        .filter_map(|item| item.value().text())
        .collect();
    assert_eq!(
        artists,
        vec!["AC/DC"],
        "a v2.4 single value with a slash must stay one value"
    );
}

/// Finding 3: the current behavior keeps an ID3v2.3 slash-separated text frame verbatim as one
/// value. This test pins that behavior until the author decides the policy.
#[test]
fn id3v23_slash_text_stays_one_value_pending_author() {
    let fixtures = TagFixtures::build();
    let path = fixtures.path(Source::Mp3);
    let output = std::process::Command::new(setup::ffmpeg())
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=duration=1",
            "-c:a",
            "libmp3lame",
            "-id3v2_version",
            "3",
            "-metadata",
            "artist=Artist One/Artist Two",
            "-metadata",
            "composer=Composer One/Composer Two",
        ])
        .arg(&path)
        .output()
        .expect("run the pinned ffmpeg");
    assert!(
        output.status.success(),
        "ffmpeg failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let tags = read_tags(&path).expect("read the source tags");
    let artists: Vec<&str> = tags
        .standard()
        .iter()
        .filter(|item| item.key() == lofty::tag::ItemKey::TrackArtist)
        .filter_map(|item| item.value().text())
        .collect();
    assert_eq!(
        artists,
        vec!["Artist One/Artist Two"],
        "the current policy keeps the v2.3 slash text as one value"
    );
}

/// The task's `encode` still writes no tag when it is given no tag set.
#[test]
fn e2e_tag_copy_writes_no_tag_without_a_tag_set() {
    let fixtures = TagFixtures::build();
    let outputs = setup::TempDir::new("melomae-tag-none");
    let audio = decode_file(fixtures.path(Source::Flac)).expect("decode the source");
    let target = outputs.join("untagged.flac");

    melomae_core::encode::encode(&audio, &target, OutputFormat::Flac).expect("encode");

    let written = read_tags(&target).expect("read the output tags");
    assert!(
        written.standard().is_empty() && written.custom().is_empty(),
        "an untagged encode must write no field"
    );
}

/// The pinned ffprobe, next to the pinned ffmpeg, as the second reader of an output.
fn ffprobe() -> std::path::PathBuf {
    let mut path = setup::ffmpeg().to_path_buf();
    path.set_file_name(if cfg!(windows) {
        "ffprobe.exe"
    } else {
        "ffprobe"
    });
    path
}

/// Read every output back with the pinned ffprobe. This is the independent reader, so a field that
/// only lofty could see would not pass. The dump goes to stdout; run with `--nocapture` to keep it.
#[test]
fn e2e_tag_copy_outputs_read_back_in_ffprobe() {
    let fixtures = TagFixtures::build();
    let outputs = setup::TempDir::new("melomae-tag-ffprobe");

    for (source, family) in fixtures.sources() {
        let path = fixtures.path(source);
        let tags = read_tags(&path).expect("read the source tags");
        let audio = decode_file(&path).expect("decode the source");
        for (format, _kind, extension) in OUTPUTS {
            let target = outputs.join(&format!("{family}.{extension}"));
            let (_, _) = encode_with_tags(&audio, &target, format, Some(&tags)).expect("encode");

            let output = std::process::Command::new(ffprobe())
                .args([
                    "-hide_banner",
                    "-v",
                    "error",
                    "-show_entries",
                    // An Ogg reader reports its comments at the stream level, and every other
                    // container at the format level, so read both.
                    "format_tags:stream_tags",
                    "-of",
                    "default=noprint_wrappers=1",
                ])
                .arg(&target)
                .output()
                .expect("run the pinned ffprobe");
            let dump = String::from_utf8_lossy(&output.stdout).into_owned();
            println!(
                "--- ffprobe {} ({family} -> {extension})\n{dump}",
                target.display()
            );

            assert!(
                output.status.success(),
                "{family} -> {extension}: ffprobe failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            for needle in [TITLE, ALBUM_ARTIST, CUSTOM_VALUE] {
                assert!(
                    dump.contains(needle),
                    "{family} -> {extension}: ffprobe did not report {needle:?}\n{dump}"
                );
            }
        }
    }
}

/// Build a one-second MP3 and replace its ID3v2 tag with `build`.
fn write_id3(path: &Path, build: impl FnOnce(&mut Id3v2Tag)) {
    let output = std::process::Command::new(setup::ffmpeg())
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=duration=1",
            "-c:a",
            "libmp3lame",
        ])
        .arg(path)
        .output()
        .expect("run the pinned ffmpeg");
    assert!(
        output.status.success(),
        "ffmpeg failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let mut tag = Id3v2Tag::new();
    build(&mut tag);
    tag.save_to_path(path, WriteOptions::default())
        .expect("write the ID3 tag");
}

/// A UTF-8 ID3v2 text frame. A value with a null separator becomes a v2.4 multi-value frame.
fn text_frame(id: &str, value: &str) -> Frame<'static> {
    Frame::Text(TextInformationFrame::new(
        FrameId::Valid(id.to_string().into()),
        TextEncoding::UTF8,
        value.to_string(),
    ))
}
