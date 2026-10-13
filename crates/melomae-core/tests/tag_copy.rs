//! Copy every tag and picture from a source file into each output format.
//!
//! The three source fixtures are built in `tag_fixtures` with the pinned ffmpeg and lofty.
//! Each source copies to FLAC, WAV, and Opus. Every source field and the front cover must survive,
//! or appear in the returned list of fields the target cannot hold.
//!
//! The expected source manifest is checked against a native reader before the copier runs, so a
//! field the fixture never wrote, or that lofty's generic layer loses, is caught. Each output is
//! then checked against the same manifest and the literal target name.

mod common;
mod setup;

#[path = "setup/tag_fixtures.rs"]
mod tag_fixtures;

use std::path::Path;

use lofty::config::{ParseOptions, WriteOptions};
use lofty::file::AudioFile;
use lofty::id3::v2::{
    BinaryFrame, CommentFrame, Frame, FrameId, Id3v2Tag, KeyValueFrame, PopularimeterFrame,
    TextInformationFrame, UniqueFileIdentifierFrame, UnsynchronizedTextFrame, UrlLinkFrame,
};
use lofty::picture::PictureType;
use lofty::prelude::TagExt;
use lofty::tag::TagType;
use lofty::TextEncoding;

use melomae_core::decode::decode_file;
use melomae_core::encode::{encode, encode_with_tags, OutputFormat};
use melomae_core::tag_copy::{read_tags, TagSet};

use crate::tag_fixtures::{
    expected_manifest, native_keys, native_manifest, native_pairs, source_kind, NativeKind, Source,
    TagFixtures, ALBUM_ARTIST, ALBUM_ARTIST_SORT, COMPOSERS, CUSTOM_NAME, CUSTOM_VALUE,
    ORIGINAL_DATE, ORIGINAL_YEAR, RELEASE_STATUS, RELEASE_STATUS_NATIVE, SCRIPT, TITLE,
};

/// The output formats, with the native tag kind and a file-name extension.
const OUTPUTS: [(OutputFormat, NativeKind, &str); 3] = [
    (OutputFormat::Flac, NativeKind::Flac, "flac"),
    (OutputFormat::Wav, NativeKind::Wav, "wav"),
    (OutputFormat::Opus, NativeKind::Opus, "opus"),
];

/// The encoder-settings value. Picard maps ID3 `TSSE` to Vorbis `ENCODERSETTINGS`. Made up, like the
/// other fixture values.
const ENCODER_SETTINGS: &str = "Fixture Encoder Settings";

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
                    Some(tag_fixtures::COVER_DESCRIPTION),
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

/// Fix round 2, finding 1: the encoder settings keep the Picard identity. An ID3 `TSSE` source and
/// a Vorbis `ENCODERSETTINGS` source both write the literal Vorbis key `ENCODERSETTINGS` in FLAC
/// and Opus, and the ID3 `TSSE` frame in WAV. Lofty's own Vorbis key, `ENCODING`, must not appear.
#[test]
fn picard_encoder_settings_keeps_its_identity_in_every_output() {
    let fixtures = TagFixtures::build();
    let outputs = setup::TempDir::new("melomae-tag-encodersettings");

    // An ID3 source carries the encoder settings in a native `TSSE` frame; a Vorbis source carries
    // them in a native `ENCODERSETTINGS` comment. Both must reach every output unchanged.
    append_id3(&fixtures.path(Source::Mp3), |tag| {
        tag.insert(text_frame("TSSE", ENCODER_SETTINGS));
    });
    append_vorbis(
        &fixtures.path(Source::Flac),
        "ENCODERSETTINGS",
        ENCODER_SETTINGS,
    );

    for (source, kind, native_key) in [
        (Source::Mp3, NativeKind::Mp3, "TSSE"),
        (Source::Flac, NativeKind::Flac, "ENCODERSETTINGS"),
    ] {
        let path = fixtures.path(source);
        assert!(
            native_keys(&path, kind).iter().any(|key| key == native_key),
            "{source:?}: the fixture must hold a native {native_key} field"
        );
        assert!(
            native_manifest(&path, kind)
                .contains(&("ENCODERSETTINGS".to_string(), ENCODER_SETTINGS.to_string())),
            "{source:?}: the native source must read as ENCODERSETTINGS={ENCODER_SETTINGS:?}"
        );

        let tags = read_tags(&path).expect("read the source tags");
        let audio = decode_file(&path).expect("decode the source");

        for (format, out_kind, extension) in OUTPUTS {
            let target = outputs.join(&format!("{source:?}.{extension}"));
            let (_, unsupported) = encode_with_tags(&audio, &target, format, Some(&tags))
                .unwrap_or_else(|error| panic!("{source:?} -> {extension}: {error}"));
            let keys = native_keys(&target, out_kind);
            let written = native_manifest(&target, out_kind);

            assert!(
                written.contains(&("ENCODERSETTINGS".to_string(), ENCODER_SETTINGS.to_string())),
                "{source:?} -> {extension}: the encoder settings must survive as ENCODERSETTINGS, got {keys:?}"
            );
            if format == OutputFormat::Wav {
                assert!(
                    keys.iter().any(|key| key == "TSSE"),
                    "{source:?} -> {extension}: a WAV output keeps the TSSE frame, got {keys:?}"
                );
            } else {
                assert!(
                    !keys.iter().any(|key| key == "ENCODING"),
                    "{source:?} -> {extension}: lofty's ENCODING alias must not appear, got {keys:?}"
                );
            }
            assert!(
                !unsupported.contains(&"ENCODERSETTINGS".to_string()),
                "{source:?} -> {extension}: the encoder settings are supported, got {unsupported:?}"
            );
        }
    }
}

/// Fix round 2, ruling 2: two encoder-settings values in a Vorbis source stay two values in the
/// Vorbis targets.
#[test]
fn two_encoder_settings_values_stay_two_in_vorbis_targets() {
    let fixtures = TagFixtures::build();
    let outputs = setup::TempDir::new("melomae-tag-encodersettings-two");
    let path = fixtures.path(Source::Flac);
    append_vorbis(&path, "ENCODERSETTINGS", "Fixture Settings One");
    append_vorbis(&path, "ENCODERSETTINGS", "Fixture Settings Two");

    let tags = read_tags(&path).expect("read the source tags");
    let audio = decode_file(&path).expect("decode the source");

    for (format, kind, extension) in [
        (OutputFormat::Flac, NativeKind::Flac, "flac"),
        (OutputFormat::Opus, NativeKind::Opus, "opus"),
    ] {
        let target = outputs.join(&format!("two.{extension}"));
        let (_, unsupported) = encode_with_tags(&audio, &target, format, Some(&tags))
            .unwrap_or_else(|error| panic!("-> {extension}: {error}"));
        let written = native_manifest(&target, kind);
        for value in ["Fixture Settings One", "Fixture Settings Two"] {
            assert!(
                written.contains(&("ENCODERSETTINGS".to_string(), value.to_string())),
                "{extension}: the encoder setting {value:?} must survive, got {written:?}"
            );
        }
        assert!(
            !native_keys(&target, kind)
                .iter()
                .any(|key| key == "ENCODING"),
            "{extension}: lofty's ENCODING alias must not appear"
        );
        assert!(unsupported.is_empty(), "{extension}: got {unsupported:?}");
    }
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

// --- Fix round 3: the full Picard tag-mapping audit -------------------------------------------

/// The two-line lyrics text the lyrics tests use.
const LYRICS_TEXT: &str = "The first line\nThe second line";
/// A musical-key value.
const KEY_VALUE: &str = "Am";
/// An integer BPM value.
const BPM_INTEGER: &str = "120";
/// A fractional BPM value that ID3 `TBPM` cannot hold.
const BPM_FRACTIONAL: &str = "120.5";
/// A composer sort-order value.
const COMPOSER_SORT: &str = "Composer, The";
/// The payload of a synchronized-lyrics frame. It must never become plain lyrics.
const SYLT_PAYLOAD: &str = "SYNCED-PAYLOAD-NOT-PLAIN-LYRICS";

/// One row of the live Picard tag mapping that has both an ID3v2 and a Vorbis entry.
///
/// `id3` is the native ID3v2 name: a text frame id such as `TIT2`, or a `TXXX` description such as
/// `BARCODE` when `user_text` is set. `wav` is the raw ID3 key the native reader reports for the
/// row in an ID3 output. `vorbis` is the documented Vorbis key.
struct PicardRow {
    label: &'static str,
    id3: &'static str,
    user_text: bool,
    vorbis: &'static str,
    value: &'static str,
    wav: &'static str,
}

/// Every row of the live Picard mapping that carries both an ID3v2 and a Vorbis entry and whose
/// ID3 side is a plain text frame or a `TXXX` frame.
///
/// Source: <https://picard-docs.musicbrainz.org/en/latest/_static/MusicBrainz_Picard_Tag_Map.html>,
/// checked 2026-10-13. Rows whose ID3 side is an involved-people role (`TIPL`/`TMCL`), a URL frame,
/// `UFID`, `COMM`, `POPM`, or whose Vorbis key carries a parameter or a second key, are recorded in
/// `evidence-15/picard-tag-mapping.md` and reported to the orchestrator instead of asserted here.
const PICARD_ROWS: &[PicardRow] = &[
    PicardRow {
        label: "AcoustID",
        id3: "Acoustid Id",
        user_text: true,
        vorbis: "ACOUSTID_ID",
        value: "89f7b982-f29d-4ad1-b442-f904907ad8b1",
        wav: "TXXX:Acoustid Id",
    },
    PicardRow {
        label: "AcoustID Fingerprint",
        id3: "Acoustid Fingerprint",
        user_text: true,
        vorbis: "ACOUSTID_FINGERPRINT",
        value: "AQAA-test",
        wav: "TXXX:Acoustid Fingerprint",
    },
    PicardRow {
        label: "Album",
        id3: "TALB",
        user_text: false,
        vorbis: "ALBUM",
        value: "Audit Album",
        wav: "TALB",
    },
    PicardRow {
        label: "Album Artist",
        id3: "TPE2",
        user_text: false,
        vorbis: "ALBUMARTIST",
        value: "Audit Album Artist",
        wav: "TPE2",
    },
    PicardRow {
        label: "Album Artist Sort Order",
        id3: "TSO2",
        user_text: false,
        vorbis: "ALBUMARTISTSORT",
        value: "Audit Album Artist Sort",
        wav: "TSO2",
    },
    PicardRow {
        label: "Album Artists",
        id3: "ALBUMARTISTS",
        user_text: true,
        vorbis: "ALBUMARTISTS",
        value: "Audit Album Artists",
        wav: "TXXX:ALBUMARTISTS",
    },
    PicardRow {
        label: "Album Sort Order",
        id3: "TSOA",
        user_text: false,
        vorbis: "ALBUMSORT",
        value: "Audit Album Sort",
        wav: "TSOA",
    },
    PicardRow {
        label: "Artist",
        id3: "TPE1",
        user_text: false,
        vorbis: "ARTIST",
        value: "Audit Artist",
        wav: "TPE1",
    },
    PicardRow {
        label: "Artist Sort Order",
        id3: "TSOP",
        user_text: false,
        vorbis: "ARTISTSORT",
        value: "Audit Artist Sort",
        wav: "TSOP",
    },
    PicardRow {
        label: "Artists",
        id3: "Artists",
        user_text: true,
        vorbis: "ARTISTS",
        value: "Audit Artists",
        wav: "TXXX:Artists",
    },
    PicardRow {
        label: "ASIN",
        id3: "ASIN",
        user_text: true,
        vorbis: "ASIN",
        value: "Audit ASIN",
        wav: "TXXX:ASIN",
    },
    PicardRow {
        label: "Barcode",
        id3: "BARCODE",
        user_text: true,
        vorbis: "BARCODE",
        value: "Audit Barcode",
        wav: "TXXX:BARCODE",
    },
    PicardRow {
        label: "BPM",
        id3: "TBPM",
        user_text: false,
        vorbis: "BPM",
        value: BPM_INTEGER,
        wav: "TBPM",
    },
    PicardRow {
        label: "Catalog Number",
        id3: "CATALOGNUMBER",
        user_text: true,
        vorbis: "CATALOGNUMBER",
        value: "Audit Catalog",
        wav: "TXXX:CATALOGNUMBER",
    },
    PicardRow {
        label: "Compilation",
        id3: "TCMP",
        user_text: false,
        vorbis: "COMPILATION",
        value: "1",
        wav: "TCMP",
    },
    PicardRow {
        label: "Composer",
        id3: "TCOM",
        user_text: false,
        vorbis: "COMPOSER",
        value: "Audit Composer",
        wav: "TCOM",
    },
    PicardRow {
        label: "Composer Sort Order",
        id3: "TSOC",
        user_text: false,
        vorbis: "COMPOSERSORT",
        value: COMPOSER_SORT,
        wav: "TSOC",
    },
    PicardRow {
        label: "Conductor",
        id3: "TPE3",
        user_text: false,
        vorbis: "CONDUCTOR",
        value: "Audit Conductor",
        wav: "TPE3",
    },
    PicardRow {
        label: "Copyright",
        id3: "TCOP",
        user_text: false,
        vorbis: "COPYRIGHT",
        value: "Audit Copyright",
        wav: "TCOP",
    },
    PicardRow {
        label: "Director",
        id3: "DIRECTOR",
        user_text: true,
        vorbis: "DIRECTOR",
        value: "Audit Director",
        wav: "TXXX:DIRECTOR",
    },
    PicardRow {
        label: "Disc Subtitle",
        id3: "TSST",
        user_text: false,
        vorbis: "DISCSUBTITLE",
        value: "Audit Disc Subtitle",
        wav: "TSST",
    },
    PicardRow {
        label: "Encoded By",
        id3: "TENC",
        user_text: false,
        vorbis: "ENCODEDBY",
        value: "Audit Encoded By",
        wav: "TENC",
    },
    PicardRow {
        label: "Encoder Settings",
        id3: "TSSE",
        user_text: false,
        vorbis: "ENCODERSETTINGS",
        value: "Audit Encoder Settings",
        wav: "TSSE",
    },
    PicardRow {
        label: "Genre",
        id3: "TCON",
        user_text: false,
        vorbis: "GENRE",
        value: "Audit Genre",
        wav: "TCON",
    },
    PicardRow {
        label: "Grouping",
        id3: "TIT1",
        user_text: false,
        vorbis: "GROUPING",
        value: "Audit Grouping",
        wav: "TIT1",
    },
    PicardRow {
        label: "Initial Key",
        id3: "TKEY",
        user_text: false,
        vorbis: "KEY",
        value: KEY_VALUE,
        wav: "TKEY",
    },
    PicardRow {
        label: "ISRC",
        id3: "TSRC",
        user_text: false,
        vorbis: "ISRC",
        value: "AUDIT0000001",
        wav: "TSRC",
    },
    PicardRow {
        label: "Language",
        id3: "TLAN",
        user_text: false,
        vorbis: "LANGUAGE",
        value: "eng",
        wav: "TLAN",
    },
    PicardRow {
        label: "Lyricist",
        id3: "TEXT",
        user_text: false,
        vorbis: "LYRICIST",
        value: "Audit Lyricist",
        wav: "TEXT",
    },
    PicardRow {
        label: "License",
        id3: "LICENSE",
        user_text: true,
        vorbis: "LICENSE",
        value: "Audit License",
        wav: "TXXX:LICENSE",
    },
    PicardRow {
        label: "Media",
        id3: "TMED",
        user_text: false,
        vorbis: "MEDIA",
        value: "Audit Media",
        wav: "TMED",
    },
    PicardRow {
        label: "Mood",
        id3: "TMOO",
        user_text: false,
        vorbis: "MOOD",
        value: "Audit Mood",
        wav: "TMOO",
    },
    PicardRow {
        label: "Movement",
        id3: "MVNM",
        user_text: false,
        vorbis: "MOVEMENTNAME",
        value: "Audit Movement",
        wav: "MVNM",
    },
    PicardRow {
        label: "MusicBrainz Artist ID",
        id3: "MusicBrainz Artist Id",
        user_text: true,
        vorbis: "MUSICBRAINZ_ARTISTID",
        value: "audit-artist-id",
        wav: "TXXX:MusicBrainz Artist Id",
    },
    PicardRow {
        label: "MusicBrainz Composer ID",
        id3: "MusicBrainz Composer Id",
        user_text: true,
        vorbis: "MUSICBRAINZ_COMPOSERID",
        value: "audit-composer-id",
        wav: "TXXX:MusicBrainz Composer Id",
    },
    PicardRow {
        label: "MusicBrainz Disc ID",
        id3: "MusicBrainz Disc Id",
        user_text: true,
        vorbis: "MUSICBRAINZ_DISCID",
        value: "audit-disc-id",
        wav: "TXXX:MusicBrainz Disc Id",
    },
    PicardRow {
        label: "MusicBrainz Original Artist ID",
        id3: "MusicBrainz Original Artist Id",
        user_text: true,
        vorbis: "MUSICBRAINZ_ORIGINALARTISTID",
        value: "audit-original-artist-id",
        wav: "TXXX:MusicBrainz Original Artist Id",
    },
    PicardRow {
        label: "MusicBrainz Original Release ID",
        id3: "MusicBrainz Original Album Id",
        user_text: true,
        vorbis: "MUSICBRAINZ_ORIGINALALBUMID",
        value: "audit-original-album-id",
        wav: "TXXX:MusicBrainz Original Album Id",
    },
    PicardRow {
        label: "MusicBrainz Release Artist ID",
        id3: "MusicBrainz Album Artist Id",
        user_text: true,
        vorbis: "MUSICBRAINZ_ALBUMARTISTID",
        value: "audit-album-artist-id",
        wav: "TXXX:MusicBrainz Album Artist Id",
    },
    PicardRow {
        label: "MusicBrainz Release Group ID",
        id3: "MusicBrainz Release Group Id",
        user_text: true,
        vorbis: "MUSICBRAINZ_RELEASEGROUPID",
        value: "audit-release-group-id",
        wav: "TXXX:MusicBrainz Release Group Id",
    },
    PicardRow {
        label: "MusicBrainz Release ID",
        id3: "MusicBrainz Album Id",
        user_text: true,
        vorbis: "MUSICBRAINZ_ALBUMID",
        value: "audit-album-id",
        wav: "TXXX:MusicBrainz Album Id",
    },
    PicardRow {
        label: "MusicBrainz Track ID",
        id3: "MusicBrainz Release Track Id",
        user_text: true,
        vorbis: "MUSICBRAINZ_RELEASETRACKID",
        value: "audit-release-track-id",
        wav: "TXXX:MusicBrainz Release Track Id",
    },
    PicardRow {
        label: "MusicBrainz TRM ID",
        id3: "MusicBrainz TRM Id",
        user_text: true,
        vorbis: "MUSICBRAINZ_TRMID",
        value: "audit-trm-id",
        wav: "TXXX:MusicBrainz TRM Id",
    },
    PicardRow {
        label: "MusicBrainz Work ID",
        id3: "MusicBrainz Work Id",
        user_text: true,
        vorbis: "MUSICBRAINZ_WORKID",
        value: "audit-work-id",
        wav: "TXXX:MusicBrainz Work Id",
    },
    PicardRow {
        label: "MusicIP PUID",
        id3: "MusicIP PUID",
        user_text: true,
        vorbis: "MUSICIP_PUID",
        value: "audit-puid",
        wav: "TXXX:MusicIP PUID",
    },
    PicardRow {
        label: "Original Filename",
        id3: "TOFN",
        user_text: false,
        vorbis: "ORIGINALFILENAME",
        value: "audit.mp3",
        wav: "TOFN",
    },
    PicardRow {
        label: "Record Label",
        id3: "TPUB",
        user_text: false,
        vorbis: "LABEL",
        value: "Audit Label",
        wav: "TPUB",
    },
    PicardRow {
        label: "Release Country",
        id3: "MusicBrainz Album Release Country",
        user_text: true,
        vorbis: "RELEASECOUNTRY",
        value: "US",
        wav: "TXXX:MusicBrainz Album Release Country",
    },
    PicardRow {
        label: "Release Status",
        id3: "MusicBrainz Album Status",
        user_text: true,
        vorbis: "RELEASESTATUS",
        value: "official",
        wav: "TXXX:MusicBrainz Album Status",
    },
    PicardRow {
        label: "Release Type",
        id3: "MusicBrainz Album Type",
        user_text: true,
        vorbis: "RELEASETYPE",
        value: "album",
        wav: "TXXX:MusicBrainz Album Type",
    },
    PicardRow {
        label: "Remixer",
        id3: "TPE4",
        user_text: false,
        vorbis: "REMIXER",
        value: "Audit Remixer",
        wav: "TPE4",
    },
    PicardRow {
        label: "ReplayGain Album Gain",
        id3: "REPLAYGAIN_ALBUM_GAIN",
        user_text: true,
        vorbis: "REPLAYGAIN_ALBUM_GAIN",
        value: "-3.5 dB",
        wav: "TXXX:REPLAYGAIN_ALBUM_GAIN",
    },
    PicardRow {
        label: "ReplayGain Album Peak",
        id3: "REPLAYGAIN_ALBUM_PEAK",
        user_text: true,
        vorbis: "REPLAYGAIN_ALBUM_PEAK",
        value: "0.987654",
        wav: "TXXX:REPLAYGAIN_ALBUM_PEAK",
    },
    PicardRow {
        label: "ReplayGain Album Range",
        id3: "REPLAYGAIN_ALBUM_RANGE",
        user_text: true,
        vorbis: "REPLAYGAIN_ALBUM_RANGE",
        value: "0.50",
        wav: "TXXX:REPLAYGAIN_ALBUM_RANGE",
    },
    PicardRow {
        label: "ReplayGain Reference Loudness",
        id3: "REPLAYGAIN_REFERENCE_LOUDNESS",
        user_text: true,
        vorbis: "REPLAYGAIN_REFERENCE_LOUDNESS",
        value: "89.0 dB",
        wav: "TXXX:REPLAYGAIN_REFERENCE_LOUDNESS",
    },
    PicardRow {
        label: "ReplayGain Track Gain",
        id3: "REPLAYGAIN_TRACK_GAIN",
        user_text: true,
        vorbis: "REPLAYGAIN_TRACK_GAIN",
        value: "-4.5 dB",
        wav: "TXXX:REPLAYGAIN_TRACK_GAIN",
    },
    PicardRow {
        label: "ReplayGain Track Peak",
        id3: "REPLAYGAIN_TRACK_PEAK",
        user_text: true,
        vorbis: "REPLAYGAIN_TRACK_PEAK",
        value: "0.876543",
        wav: "TXXX:REPLAYGAIN_TRACK_PEAK",
    },
    PicardRow {
        label: "ReplayGain Track Range",
        id3: "REPLAYGAIN_TRACK_RANGE",
        user_text: true,
        vorbis: "REPLAYGAIN_TRACK_RANGE",
        value: "5.50",
        wav: "TXXX:REPLAYGAIN_TRACK_RANGE",
    },
    PicardRow {
        label: "Script",
        id3: "SCRIPT",
        user_text: true,
        vorbis: "SCRIPT",
        value: "Latn",
        wav: "TXXX:SCRIPT",
    },
    PicardRow {
        label: "Show Work & Movement",
        id3: "SHOWMOVEMENT",
        user_text: true,
        vorbis: "SHOWMOVEMENT",
        value: "1",
        wav: "TXXX:SHOWMOVEMENT",
    },
    PicardRow {
        label: "Subtitle",
        id3: "TIT3",
        user_text: false,
        vorbis: "SUBTITLE",
        value: "Audit Subtitle",
        wav: "TIT3",
    },
    PicardRow {
        label: "Track Title",
        id3: "TIT2",
        user_text: false,
        vorbis: "TITLE",
        value: "Audit Title",
        wav: "TIT2",
    },
    PicardRow {
        label: "Track Title Sort Order",
        id3: "TSOT",
        user_text: false,
        vorbis: "TITLESORT",
        value: "Audit, Title",
        wav: "TSOT",
    },
    PicardRow {
        label: "Work Title",
        id3: "WORK",
        user_text: true,
        vorbis: "WORK",
        value: "Audit Work",
        wav: "TXXX:WORK",
    },
    PicardRow {
        label: "Writer",
        id3: "Writer",
        user_text: true,
        vorbis: "WRITER",
        value: "Audit Writer",
        wav: "TXXX:Writer",
    },
];

/// One measured output: its format, the native `(key, value)` pairs it holds, and the report of the
/// fields the target could not hold.
type MeasuredOutput = (OutputFormat, Vec<(String, String)>, Vec<String>);

/// One table-driven audit of the live Picard mapping. One row is one line of test data. Each row's
/// native ID3v2 field is written into an MP3, copied to FLAC, Opus, and WAV, and read back with a
/// native reader. The Vorbis target must carry the documented Vorbis key with the exact value, and
/// the WAV target the documented ID3 key. A row that is skipped, renamed, or altered is a failure.
#[test]
fn picard_tag_mapping_audit_holds_for_every_dual_format_row() {
    let sources = setup::TempDir::new("melomae-picard-audit-src");
    let outputs = setup::TempDir::new("melomae-picard-audit-out");
    let source = sources.join("audit.mp3");
    write_id3(&source, |tag| {
        for row in PICARD_ROWS {
            if row.user_text {
                tag.insert_user_text(row.id3.to_string(), row.value.to_string());
            } else {
                tag.insert(text_frame(row.id3, row.value));
            }
        }
    });

    // The fixture must hold every row natively, so the audit cannot start from a partial source.
    let source_pairs = native_pairs(&source, NativeKind::Mp3);
    for row in PICARD_ROWS {
        assert!(
            source_pairs.contains(&(row.wav.to_string(), row.value.to_string())),
            "{}: the fixture did not write the native source field {}={:?}",
            row.label,
            row.wav,
            row.value
        );
    }

    let tags = read_tags(&source).expect("read the audit source");
    let audio = decode_file(&source).expect("decode the audit source");

    let mut measured: Vec<MeasuredOutput> = Vec::new();
    let mut failures = Vec::new();
    for (format, kind, extension) in OUTPUTS {
        let target = outputs.join(&format!("audit.{extension}"));
        match encode_with_tags(&audio, &target, format, Some(&tags)) {
            Ok((_, report)) => measured.push((format, native_pairs(&target, kind), report)),
            Err(error) => failures.push(format!("{format:?} encode failed: {error}")),
        }
    }

    println!("--- picard audit measured ---");
    for (format, pairs, report) in &measured {
        println!("{format:?} report={report:?}\n{pairs:?}");
    }

    for row in PICARD_ROWS {
        for (format, pairs, _report) in &measured {
            let key = match format {
                OutputFormat::Wav => row.wav,
                OutputFormat::Flac | OutputFormat::Opus => row.vorbis,
            };
            if !pairs.contains(&(key.to_string(), row.value.to_string())) {
                failures.push(format!(
                    "{} -> {format:?}: expected {key}={:?}",
                    row.label, row.value
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} Picard mapping rows were not honored:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Finding 1: an ordinary ID3 `USLT` frame maps to the Vorbis `LYRICS` key, not `UNSYNCEDLYRICS`.
/// The WAV output keeps the `USLT` frame, and a Vorbis `LYRICS` source writes the `USLT` frame.
#[test]
fn picard_lyrics_use_the_literal_native_names() {
    let fixtures = TagFixtures::build();
    let outputs = setup::TempDir::new("melomae-picard-lyrics");
    let path = fixtures.path(Source::Mp3);
    append_id3(&path, |tag| {
        tag.insert(Frame::UnsynchronizedText(UnsynchronizedTextFrame::new(
            TextEncoding::UTF8,
            *b"eng",
            "",
            LYRICS_TEXT,
        )));
    });

    let mut file = std::fs::File::open(&path).expect("open the MP3");
    let mpeg = lofty::mpeg::MpegFile::read_from(&mut file, ParseOptions::new()).expect("read MP3");
    assert!(
        mpeg.id3v2()
            .map(|tag| tag
                .iter()
                .any(|frame| matches!(frame, Frame::UnsynchronizedText(text)
                if text.content == LYRICS_TEXT && text.description.is_empty())))
            .unwrap_or(false),
        "the fixture must hold a native USLT frame with its exact text"
    );

    let tags = read_tags(&path).expect("read the source tags");
    let audio = decode_file(&path).expect("decode the source");
    for (format, kind, extension) in [
        (OutputFormat::Flac, NativeKind::Flac, "flac"),
        (OutputFormat::Opus, NativeKind::Opus, "opus"),
    ] {
        let target = outputs.join(&format!("lyrics.{extension}"));
        encode_with_tags(&audio, &target, format, Some(&tags)).expect("encode");
        let pairs = native_pairs(&target, kind);
        assert!(
            pairs.contains(&("LYRICS".to_string(), LYRICS_TEXT.to_string())),
            "{extension}: USLT must map to Picard LYRICS, got {pairs:?}"
        );
        assert!(
            !pairs.iter().any(|(key, _)| key == "UNSYNCEDLYRICS"),
            "{extension}: lofty's UNSYNCEDLYRICS alias must not appear, got {pairs:?}"
        );
    }

    let target = outputs.join("lyrics.wav");
    encode_with_tags(&audio, &target, OutputFormat::Wav, Some(&tags)).expect("encode WAV");
    assert!(
        wav_has_unsynchronized_text(&target, LYRICS_TEXT),
        "the WAV output must keep the USLT frame"
    );

    let flac = fixtures.path(Source::Flac);
    append_vorbis(&flac, "LYRICS", LYRICS_TEXT);
    let tags = read_tags(&flac).expect("read the FLAC tags");
    let audio = decode_file(&flac).expect("decode the FLAC");
    let target = outputs.join("lyrics-from-vorbis.wav");
    encode_with_tags(&audio, &target, OutputFormat::Wav, Some(&tags)).expect("encode WAV");
    assert!(
        wav_has_unsynchronized_text(&target, LYRICS_TEXT),
        "a Vorbis LYRICS must write the ID3 USLT frame"
    );
}

/// Finding 2: an ID3 `TKEY` frame maps to the Vorbis `KEY` key, not `INITIALKEY`, in every target,
/// and a Vorbis `KEY` source writes the ID3 `TKEY` frame.
#[test]
fn picard_initial_key_uses_the_literal_native_names() {
    let fixtures = TagFixtures::build();
    let outputs = setup::TempDir::new("melomae-picard-key");
    let path = fixtures.path(Source::Mp3);
    append_id3(&path, |tag| {
        tag.insert(text_frame("TKEY", KEY_VALUE));
    });
    assert!(
        native_pairs(&path, NativeKind::Mp3).contains(&("TKEY".to_string(), KEY_VALUE.to_string())),
        "the fixture must hold a native TKEY frame"
    );

    let tags = read_tags(&path).expect("read the source tags");
    let audio = decode_file(&path).expect("decode the source");
    for (format, kind, extension) in OUTPUTS {
        let target = outputs.join(&format!("key.{extension}"));
        encode_with_tags(&audio, &target, format, Some(&tags)).expect("encode");
        let pairs = native_pairs(&target, kind);
        if format == OutputFormat::Wav {
            assert!(
                pairs.contains(&("TKEY".to_string(), KEY_VALUE.to_string())),
                "{extension}: a WAV output keeps the TKEY frame, got {pairs:?}"
            );
        } else {
            assert!(
                pairs.contains(&("KEY".to_string(), KEY_VALUE.to_string())),
                "{extension}: TKEY must map to Picard KEY, got {pairs:?}"
            );
            assert!(
                !pairs.iter().any(|(key, _)| key == "INITIALKEY"),
                "{extension}: INITIALKEY must not appear, got {pairs:?}"
            );
        }
    }

    let flac = fixtures.path(Source::Flac);
    append_vorbis(&flac, "KEY", KEY_VALUE);
    let tags = read_tags(&flac).expect("read the FLAC tags");
    let audio = decode_file(&flac).expect("decode the FLAC");
    let target = outputs.join("key-from-vorbis.wav");
    encode_with_tags(&audio, &target, OutputFormat::Wav, Some(&tags)).expect("encode WAV");
    assert!(
        native_pairs(&target, NativeKind::Wav)
            .contains(&("TKEY".to_string(), KEY_VALUE.to_string())),
        "a Vorbis KEY must write the ID3 TKEY frame"
    );
}

/// Finding 3: an ID3 `TBPM` frame maps to the Vorbis `BPM` key and `TSOC` to `COMPOSERSORT`, WAV
/// keeps `TBPM` and `TSOC`, a fractional BPM keeps its exact text in Vorbis and is reported for
/// WAV, and a Vorbis `COMPOSERSORT` source writes the ID3 `TSOC` frame.
#[test]
fn picard_bpm_and_composer_sort_fit_the_vorbis_outputs() {
    let fixtures = TagFixtures::build();
    let outputs = setup::TempDir::new("melomae-picard-bpm-sort");
    let path = fixtures.path(Source::Mp3);
    append_id3(&path, |tag| {
        tag.insert(text_frame("TBPM", BPM_INTEGER));
        tag.insert(text_frame("TSOC", COMPOSER_SORT));
    });
    let source_pairs = native_pairs(&path, NativeKind::Mp3);
    assert!(source_pairs.contains(&("TBPM".to_string(), BPM_INTEGER.to_string())));
    assert!(source_pairs.contains(&("TSOC".to_string(), COMPOSER_SORT.to_string())));

    let tags = read_tags(&path).expect("read the source tags");
    let audio = decode_file(&path).expect("decode the source");
    for (format, kind, extension) in OUTPUTS {
        let target = outputs.join(&format!("bpm-sort.{extension}"));
        let (_, report) = encode_with_tags(&audio, &target, format, Some(&tags)).expect("encode");
        let pairs = native_pairs(&target, kind);
        if format == OutputFormat::Wav {
            assert!(
                pairs.contains(&("TBPM".to_string(), BPM_INTEGER.to_string())),
                "{extension}: a WAV output keeps TBPM, got {pairs:?}"
            );
            assert!(
                pairs.contains(&("TSOC".to_string(), COMPOSER_SORT.to_string())),
                "{extension}: a WAV output keeps TSOC, got {pairs:?}"
            );
        } else {
            assert!(
                pairs.contains(&("BPM".to_string(), BPM_INTEGER.to_string())),
                "{extension}: TBPM must map to Picard BPM, got {pairs:?} report={report:?}"
            );
            assert!(
                pairs.contains(&("COMPOSERSORT".to_string(), COMPOSER_SORT.to_string())),
                "{extension}: TSOC must map to Picard COMPOSERSORT, got {pairs:?} report={report:?}"
            );
        }
        assert!(
            !report
                .iter()
                .any(|name| name == "IntegerBpm" || name == "ComposerSortOrder"),
            "{extension}: the fields are supported, got {report:?}"
        );
    }

    // A fractional BPM keeps its exact text in a Vorbis target, and is reported for WAV.
    let flac = fixtures.path(Source::Flac);
    append_vorbis(&flac, "BPM", BPM_FRACTIONAL);
    let tags = read_tags(&flac).expect("read the FLAC tags");
    let audio = decode_file(&flac).expect("decode the FLAC");
    for (format, kind, extension) in [
        (OutputFormat::Flac, NativeKind::Flac, "flac"),
        (OutputFormat::Opus, NativeKind::Opus, "opus"),
    ] {
        let target = outputs.join(&format!("fractional.{extension}"));
        encode_with_tags(&audio, &target, format, Some(&tags)).expect("encode");
        assert!(
            native_pairs(&target, kind).contains(&("BPM".to_string(), BPM_FRACTIONAL.to_string())),
            "{extension}: a fractional BPM must keep its exact text"
        );
    }
    let target = outputs.join("fractional.wav");
    let (_, report) =
        encode_with_tags(&audio, &target, OutputFormat::Wav, Some(&tags)).expect("encode WAV");
    assert!(
        report.contains(&"BPM".to_string()),
        "a fractional BPM that ID3 cannot hold must be reported, got {report:?}"
    );

    // A Vorbis COMPOSERSORT source writes the ID3 TSOC frame.
    append_vorbis(&flac, "COMPOSERSORT", COMPOSER_SORT);
    let tags = read_tags(&flac).expect("read the FLAC tags");
    let audio = decode_file(&flac).expect("decode the FLAC");
    let target = outputs.join("sort-from-vorbis.wav");
    encode_with_tags(&audio, &target, OutputFormat::Wav, Some(&tags)).expect("encode WAV");
    assert!(
        native_pairs(&target, NativeKind::Wav)
            .contains(&("TSOC".to_string(), COMPOSER_SORT.to_string())),
        "a Vorbis COMPOSERSORT must write the ID3 TSOC frame"
    );
}

/// Ruling 2: synchronized lyrics (`SYLT`) stay separate. The copier must never turn their binary
/// payload into plain lyrics; the frame is reported instead.
#[test]
fn picard_synchronized_lyrics_stay_separate() {
    let fixtures = TagFixtures::build();
    let outputs = setup::TempDir::new("melomae-picard-sylt");
    let path = fixtures.path(Source::Mp3);
    append_id3(&path, |tag| {
        tag.insert(Frame::Binary(BinaryFrame::new(
            FrameId::Valid("SYLT".to_string().into()),
            SYLT_PAYLOAD.as_bytes().to_vec(),
        )));
    });

    let tags = read_tags(&path).expect("read the source tags");
    let audio = decode_file(&path).expect("decode the source");
    for (format, kind, extension) in OUTPUTS {
        let target = outputs.join(&format!("sylt.{extension}"));
        let (_, report) = encode_with_tags(&audio, &target, format, Some(&tags)).expect("encode");
        let pairs = native_pairs(&target, kind);
        assert!(
            !pairs.iter().any(|(key, _)| key == "LYRICS"),
            "{extension}: synchronized lyrics must not become LYRICS, got {pairs:?}"
        );
        assert!(
            report.contains(&"SYLT".to_string()),
            "{extension}: the synchronized-lyrics frame must be reported, got {report:?}"
        );
    }
}

// --- Fix round 3, part 2: the Group A mappings and the Group B no-loss tests -------------------

/// A `UFID` owner that is not MusicBrainz.
const FOREIGN_UFID_OWNER: &str = "http://example.com";
/// A website URL.
const WEBSITE_URL: &str = "https://example.com/artist";
/// A license URL.
const LICENSE_URL: &str = "https://example.com/license";
/// The first license URL of a multi-value license field.
const LICENSE_URL_A: &str = "https://example.com/license-a";
/// The second license URL of a multi-value license field.
const LICENSE_URL_B: &str = "https://example.com/license-b";
/// A license value that is not a URL.
const LICENSE_TEXT: &str = "All rights reserved";
/// The value the Group A/B frames carry.
const FRAME_VALUE: &str = "Audit Value";

/// Read the ID3v2 tag of an MP3 file.
fn mp3_id3(path: &Path) -> Id3v2Tag {
    let mut file = std::fs::File::open(path).expect("open the MP3");
    lofty::mpeg::MpegFile::read_from(&mut file, ParseOptions::new())
        .expect("read the MP3")
        .id3v2()
        .cloned()
        .unwrap_or_default()
}

/// Read the ID3v2 tag of a WAV file.
fn wav_id3(path: &Path) -> Id3v2Tag {
    let mut file = std::fs::File::open(path).expect("open the WAV");
    lofty::iff::wav::WavFile::read_from(&mut file, ParseOptions::new())
        .expect("read the WAV")
        .id3v2()
        .cloned()
        .unwrap_or_default()
}

/// The `(owner, identifier)` of the first `UFID` frame in a tag.
fn ufid_of(tag: &Id3v2Tag) -> Option<(String, Vec<u8>)> {
    tag.iter().find_map(|frame| match frame {
        Frame::UniqueFileIdentifier(ufid) => {
            Some((ufid.owner.to_string(), ufid.identifier.to_vec()))
        }
        _ => None,
    })
}

/// The IDs of every frame in a tag.
fn frame_ids(tag: &Id3v2Tag) -> Vec<String> {
    tag.iter().map(|frame| frame.id_str().to_string()).collect()
}

/// Whether a tag holds a URL frame with the given id and content.
fn has_url(tag: &Id3v2Tag, id: &str, content: &str) -> bool {
    tag.iter().any(
        |frame| matches!(frame, Frame::Url(url) if url.id().as_str() == id && url.url() == content),
    )
}

/// The license values a native reader reports, one per value. A Vorbis `LICENSE` comment is one
/// value each. An ID3v2.4 `TXXX:LICENSE` frame carries several values null-separated in one frame,
/// so it is split here; a single URL is the `WCOP` URL frame, which is read directly.
fn native_license_values(path: &Path, kind: NativeKind) -> Vec<String> {
    let mut values = Vec::new();
    for (key, value) in native_pairs(path, kind) {
        if key.eq_ignore_ascii_case("LICENSE") || key.eq_ignore_ascii_case("TXXX:LICENSE") {
            values.extend(value.split('\0').map(str::to_string));
        }
    }
    if kind == NativeKind::Wav {
        for frame in wav_id3(path).iter() {
            if let Frame::Url(url) = frame {
                if url.id().as_str() == "WCOP" {
                    values.push(url.url().to_string());
                }
            }
        }
    }
    values
}

/// Copy one source into all three outputs and check that the license row keeps exactly `expected`
/// values in every output, read back with a native reader. The source's own native values are
/// checked first, so the test does not check the copier against itself.
fn assert_license_round_trip(source: &Path, kind: NativeKind, expected: &[&str], dir: &str) {
    let mut want: Vec<String> = expected.iter().map(|value| (*value).to_string()).collect();
    want.sort();
    let mut found = native_license_values(source, kind);
    found.sort();
    assert_eq!(
        found, want,
        "the source must hold the literal license values, got {found:?}"
    );

    let tags = read_tags(source).expect("read the license source");
    let audio = decode_file(source).expect("decode the license source");
    let outputs = setup::TempDir::new(dir);
    for (format, out_kind, extension) in OUTPUTS {
        let target = outputs.join(&format!("license.{extension}"));
        let (_, report) = encode_with_tags(&audio, &target, format, Some(&tags))
            .unwrap_or_else(|error| panic!("-> {extension}: {error}"));
        let mut got = native_license_values(&target, out_kind);
        got.sort();
        assert_eq!(
            got, want,
            "-> {extension}: every license value must survive, report={report:?}"
        );
    }
}

/// Ruling on the 10 unclear rows, Group A1: the MusicBrainz `UFID` maps to the Vorbis
/// `MUSICBRAINZ_TRACKID` key and back. A `UFID` with any other owner is preserved in WAV and named
/// for a Vorbis target, so it is never silently dropped or renamed.
#[test]
fn picard_recording_id_maps_the_musicbrainz_ufid() {
    let dir = setup::TempDir::new("melomae-picard-ufid");

    let musicbrainz = dir.join("mb.mp3");
    write_id3(&musicbrainz, |tag| {
        tag.insert(Frame::UniqueFileIdentifier(UniqueFileIdentifierFrame::new(
            "http://musicbrainz.org",
            b"audit-recording-id".to_vec(),
        )));
    });
    assert!(
        ufid_of(&mp3_id3(&musicbrainz)).is_some_and(|(owner, id)| owner
            == "http://musicbrainz.org"
            && id == b"audit-recording-id"),
        "the fixture must hold the MusicBrainz UFID"
    );
    let tags = read_tags(&musicbrainz).expect("read the source tags");
    let audio = decode_file(&musicbrainz).expect("decode the source");
    for (format, kind, extension) in [
        (OutputFormat::Flac, NativeKind::Flac, "flac"),
        (OutputFormat::Opus, NativeKind::Opus, "opus"),
    ] {
        let target = dir.join(&format!("mb.{extension}"));
        encode_with_tags(&audio, &target, format, Some(&tags)).expect("encode");
        assert!(
            native_pairs(&target, kind).contains(&(
                "MUSICBRAINZ_TRACKID".to_string(),
                "audit-recording-id".to_string()
            )),
            "{extension}: the recording ID must become MUSICBRAINZ_TRACKID"
        );
    }
    let target = dir.join("mb.wav");
    encode_with_tags(&audio, &target, OutputFormat::Wav, Some(&tags)).expect("encode WAV");
    assert!(
        ufid_of(&wav_id3(&target)).is_some_and(
            |(owner, id)| owner == "http://musicbrainz.org" && id == b"audit-recording-id"
        ),
        "the WAV output must keep the MusicBrainz UFID"
    );

    let foreign = dir.join("foreign.mp3");
    write_id3(&foreign, |tag| {
        tag.insert(Frame::UniqueFileIdentifier(UniqueFileIdentifierFrame::new(
            FOREIGN_UFID_OWNER,
            b"audit-owner-id".to_vec(),
        )));
    });
    let tags = read_tags(&foreign).expect("read the foreign tags");
    let audio = decode_file(&foreign).expect("decode the source");
    for (format, kind, extension) in [
        (OutputFormat::Flac, NativeKind::Flac, "flac"),
        (OutputFormat::Opus, NativeKind::Opus, "opus"),
    ] {
        let target = dir.join(&format!("foreign.{extension}"));
        let (_, report) = encode_with_tags(&audio, &target, format, Some(&tags)).expect("encode");
        assert!(
            report.contains(&"UFID".to_string()),
            "{extension}: a foreign UFID must be named, got {report:?}"
        );
        assert!(
            !native_pairs(&target, kind)
                .iter()
                .any(|(key, _)| key == "MUSICBRAINZ_TRACKID"),
            "{extension}: a foreign UFID must not become MUSICBRAINZ_TRACKID"
        );
    }
    let target = dir.join("foreign.wav");
    encode_with_tags(&audio, &target, OutputFormat::Wav, Some(&tags)).expect("encode WAV");
    assert!(
        ufid_of(&wav_id3(&target))
            .is_some_and(|(owner, id)| owner == FOREIGN_UFID_OWNER && id == b"audit-owner-id"),
        "the WAV output must keep the foreign UFID unchanged"
    );
}

/// Group A2: an ID3 `WOAR` frame maps to Vorbis `WEBSITE` and back, and the WAV output keeps the
/// `WOAR` frame.
#[test]
fn picard_website_maps_the_woar_frame() {
    let fixtures = TagFixtures::build();
    let dir = setup::TempDir::new("melomae-picard-website");
    let path = fixtures.path(Source::Mp3);
    append_id3(&path, |tag| {
        tag.insert(Frame::Url(UrlLinkFrame::new(
            FrameId::Valid("WOAR".into()),
            WEBSITE_URL,
        )));
    });
    let tags = read_tags(&path).expect("read the source tags");
    let audio = decode_file(&path).expect("decode the source");
    for (format, kind, extension) in [
        (OutputFormat::Flac, NativeKind::Flac, "flac"),
        (OutputFormat::Opus, NativeKind::Opus, "opus"),
    ] {
        let target = dir.join(&format!("website.{extension}"));
        encode_with_tags(&audio, &target, format, Some(&tags)).expect("encode");
        assert!(
            native_pairs(&target, kind).contains(&("WEBSITE".to_string(), WEBSITE_URL.to_string())),
            "{extension}: WOAR must become the Vorbis WEBSITE key"
        );
    }
    let target = dir.join("website.wav");
    encode_with_tags(&audio, &target, OutputFormat::Wav, Some(&tags)).expect("encode WAV");
    assert!(
        has_url(&wav_id3(&target), "WOAR", WEBSITE_URL),
        "the WAV output must keep the WOAR frame"
    );

    // And back: a Vorbis WEBSITE source writes the WOAR frame.
    let flac = fixtures.path(Source::Flac);
    append_vorbis(&flac, "WEBSITE", WEBSITE_URL);
    let tags = read_tags(&flac).expect("read the FLAC tags");
    let audio = decode_file(&flac).expect("decode the FLAC");
    let target = dir.join("website-from-vorbis.wav");
    encode_with_tags(&audio, &target, OutputFormat::Wav, Some(&tags)).expect("encode WAV");
    assert!(
        has_url(&wav_id3(&target), "WOAR", WEBSITE_URL),
        "a Vorbis WEBSITE must write the ID3 WOAR frame"
    );
}

/// Group A3: an ID3 `WCOP` frame and an ID3 `TXXX:LICENSE` frame both map to Vorbis `LICENSE`; a
/// URL license writes `WCOP` in WAV, and a non-URL license keeps the `TXXX:LICENSE` description.
#[test]
fn picard_license_maps_wcop_and_txxx() {
    let fixtures = TagFixtures::build();
    let dir = setup::TempDir::new("melomae-picard-license");

    let wcop = dir.join("wcop.mp3");
    write_id3(&wcop, |tag| {
        tag.insert(Frame::Url(UrlLinkFrame::new(
            FrameId::Valid("WCOP".into()),
            LICENSE_URL,
        )));
    });
    let tags = read_tags(&wcop).expect("read the source tags");
    let audio = decode_file(&wcop).expect("decode the source");
    for (format, kind, extension) in [
        (OutputFormat::Flac, NativeKind::Flac, "flac"),
        (OutputFormat::Opus, NativeKind::Opus, "opus"),
    ] {
        let target = dir.join(&format!("wcop.{extension}"));
        encode_with_tags(&audio, &target, format, Some(&tags)).expect("encode");
        assert!(
            native_pairs(&target, kind).contains(&("LICENSE".to_string(), LICENSE_URL.to_string())),
            "{extension}: WCOP must become the Vorbis LICENSE key"
        );
    }
    let target = dir.join("wcop.wav");
    encode_with_tags(&audio, &target, OutputFormat::Wav, Some(&tags)).expect("encode WAV");
    assert!(
        has_url(&wav_id3(&target), "WCOP", LICENSE_URL),
        "the WAV output must keep the WCOP frame"
    );

    // A TXXX:LICENSE source maps to Vorbis LICENSE, and a URL writes WCOP in WAV.
    let txxx = dir.join("txxx.mp3");
    write_id3(&txxx, |tag| {
        tag.insert_user_text("LICENSE".to_string(), LICENSE_URL.to_string());
    });
    let tags = read_tags(&txxx).expect("read the TXXX tags");
    let audio = decode_file(&txxx).expect("decode");
    for (format, kind, extension) in [
        (OutputFormat::Flac, NativeKind::Flac, "flac"),
        (OutputFormat::Opus, NativeKind::Opus, "opus"),
    ] {
        let target = dir.join(&format!("txxx.{extension}"));
        encode_with_tags(&audio, &target, format, Some(&tags)).expect("encode");
        assert!(
            native_pairs(&target, kind).contains(&("LICENSE".to_string(), LICENSE_URL.to_string())),
            "{extension}: TXXX:LICENSE must become the Vorbis LICENSE key"
        );
    }
    let target = dir.join("txxx.wav");
    encode_with_tags(&audio, &target, OutputFormat::Wav, Some(&tags)).expect("encode WAV");
    assert!(
        has_url(&wav_id3(&target), "WCOP", LICENSE_URL),
        "a URL license must write the WCOP frame in WAV"
    );

    // A non-URL Vorbis LICENSE keeps the TXXX:LICENSE description.
    let flac = fixtures.path(Source::Flac);
    append_vorbis(&flac, "LICENSE", LICENSE_TEXT);
    let tags = read_tags(&flac).expect("read the FLAC tags");
    let audio = decode_file(&flac).expect("decode the FLAC");
    let target = dir.join("text.wav");
    encode_with_tags(&audio, &target, OutputFormat::Wav, Some(&tags)).expect("encode WAV");
    assert!(
        native_pairs(&target, NativeKind::Wav)
            .contains(&("TXXX:LICENSE".to_string(), LICENSE_TEXT.to_string())),
        "a non-URL license must keep the TXXX:LICENSE description"
    );
}

/// Finding 1 of re-review 4: the license row keeps every value. A Vorbis source with two URL-only
/// license values and with a URL plus a text value must copy all of them into FLAC, Opus, and WAV.
/// Two or more values become one `TXXX:LICENSE` frame whose values are null-separated, so the native
/// reader is checked for the exact values and their multiplicity.
#[test]
fn picard_license_keeps_every_value_from_vorbis() {
    for values in [
        [LICENSE_URL_A, LICENSE_URL_B],
        [LICENSE_URL_A, LICENSE_TEXT],
    ] {
        let fixtures = TagFixtures::build();
        let source = fixtures.path(Source::Flac);
        for value in &values {
            append_vorbis(&source, "LICENSE", value);
        }
        assert_license_round_trip(&source, NativeKind::Flac, &values, "melomae-license-vorbis");
    }
}

/// Finding 1 of re-review 4: the same, from an MP3 source whose `TXXX:LICENSE` frame carries the
/// values. The reader splits the frame, so both the values and the count must survive a copy.
#[test]
fn picard_license_keeps_every_value_from_id3() {
    for values in [
        [LICENSE_URL_A, LICENSE_URL_B],
        [LICENSE_URL_A, LICENSE_TEXT],
    ] {
        let dir = setup::TempDir::new("melomae-license-id3");
        let source = dir.join("license.mp3");
        write_id3(&source, |tag| {
            tag.insert_user_text("LICENSE".to_string(), values.join("\0"));
        });
        assert_license_round_trip(&source, NativeKind::Mp3, &values, "melomae-license-id3-out");
    }
}

/// Group A4: `MVIN` holds `number` or `number/count`, becomes Vorbis `MOVEMENT` and
/// `MOVEMENTTOTAL`, joins back into one `MVIN` in WAV, and a malformed value is reported.
#[test]
fn picard_movement_maps_mvin() {
    let fixtures = TagFixtures::build();
    let dir = setup::TempDir::new("melomae-picard-movement");

    // `number/count`.
    let path = fixtures.path(Source::Mp3);
    append_id3(&path, |tag| {
        tag.insert(text_frame("MVIN", "2/5"));
    });
    let tags = read_tags(&path).expect("read the source tags");
    let audio = decode_file(&path).expect("decode the source");
    for (format, kind, extension) in [
        (OutputFormat::Flac, NativeKind::Flac, "flac"),
        (OutputFormat::Opus, NativeKind::Opus, "opus"),
    ] {
        let target = dir.join(&format!("pair.{extension}"));
        let (_, report) = encode_with_tags(&audio, &target, format, Some(&tags)).expect("encode");
        let pairs = native_pairs(&target, kind);
        assert!(
            pairs.contains(&("MOVEMENT".to_string(), "2".to_string()))
                && pairs.contains(&("MOVEMENTTOTAL".to_string(), "5".to_string())),
            "{extension}: 2/5 must become MOVEMENT=2 and MOVEMENTTOTAL=5, got {pairs:?} report={report:?}"
        );
    }
    let target = dir.join("pair.wav");
    encode_with_tags(&audio, &target, OutputFormat::Wav, Some(&tags)).expect("encode WAV");
    assert!(
        native_pairs(&target, NativeKind::Wav).contains(&("MVIN".to_string(), "2/5".to_string())),
        "the WAV output must join the movement back into one MVIN=2/5"
    );

    // A bare number.
    let bare = dir.join("bare.mp3");
    write_id3(&bare, |tag| {
        tag.insert(text_frame("MVIN", "7"));
    });
    let tags = read_tags(&bare).expect("read the bare tags");
    let audio = decode_file(&bare).expect("decode");
    let target = dir.join("bare.flac");
    encode_with_tags(&audio, &target, OutputFormat::Flac, Some(&tags)).expect("encode");
    assert!(native_pairs(&target, NativeKind::Flac)
        .contains(&("MOVEMENT".to_string(), "7".to_string())));
    let target = dir.join("bare.wav");
    encode_with_tags(&audio, &target, OutputFormat::Wav, Some(&tags)).expect("encode WAV");
    assert!(
        native_pairs(&target, NativeKind::Wav).contains(&("MVIN".to_string(), "7".to_string())),
        "a bare movement must stay one MVIN=7"
    );

    // A malformed value is reported, not guessed.
    let bad = dir.join("bad.mp3");
    write_id3(&bad, |tag| {
        tag.insert(text_frame("MVIN", "not-a-number"));
    });
    let tags = read_tags(&bad).expect("read the bad tags");
    let audio = decode_file(&bad).expect("decode");
    for (format, kind, extension) in [
        (OutputFormat::Flac, NativeKind::Flac, "flac"),
        (OutputFormat::Opus, NativeKind::Opus, "opus"),
    ] {
        let target = dir.join(&format!("bad.{extension}"));
        let (_, report) = encode_with_tags(&audio, &target, format, Some(&tags)).expect("encode");
        assert!(
            report.contains(&"MVIN".to_string()),
            "{extension}: a malformed MVIN must be reported, got {report:?}"
        );
        assert!(
            native_pairs(&target, kind).is_empty(),
            "{extension}: a malformed MVIN must not be guessed into a Vorbis key"
        );
    }
    let target = dir.join("bad.wav");
    encode_with_tags(&audio, &target, OutputFormat::Wav, Some(&tags)).expect("encode WAV");
    assert!(
        native_pairs(&target, NativeKind::Wav)
            .contains(&("MVIN".to_string(), "not-a-number".to_string())),
        "the WAV output must keep the malformed MVIN unchanged"
    );

    // The reverse: Vorbis MOVEMENT and MOVEMENTTOTAL join back into one MVIN.
    let flac = fixtures.path(Source::Flac);
    append_vorbis(&flac, "MOVEMENT", "3");
    append_vorbis(&flac, "MOVEMENTTOTAL", "9");
    let tags = read_tags(&flac).expect("read the FLAC tags");
    let audio = decode_file(&flac).expect("decode the FLAC");
    let target = dir.join("from-vorbis.wav");
    encode_with_tags(&audio, &target, OutputFormat::Wav, Some(&tags)).expect("encode WAV");
    assert!(
        native_pairs(&target, NativeKind::Wav).contains(&("MVIN".to_string(), "3/9".to_string())),
        "Vorbis MOVEMENT/MOVEMENTTOTAL must join into one MVIN=3/9"
    );
}

/// Group A5: a `COMM` with an empty description maps to Vorbis `COMMENT`; a `COMM` with a
/// description is preserved in WAV and named for a Vorbis target, never merged into `COMMENT`.
#[test]
fn picard_comment_keeps_the_description() {
    let fixtures = TagFixtures::build();
    let dir = setup::TempDir::new("melomae-picard-comment");

    let path = fixtures.path(Source::Mp3);
    append_id3(&path, |tag| {
        tag.insert(Frame::Comment(CommentFrame::new(
            TextEncoding::UTF8,
            *b"eng",
            "",
            "Audit comment",
        )));
    });
    let tags = read_tags(&path).expect("read the source tags");
    let audio = decode_file(&path).expect("decode the source");
    for (format, kind, extension) in [
        (OutputFormat::Flac, NativeKind::Flac, "flac"),
        (OutputFormat::Opus, NativeKind::Opus, "opus"),
    ] {
        let target = dir.join(&format!("plain.{extension}"));
        encode_with_tags(&audio, &target, format, Some(&tags)).expect("encode");
        assert!(
            native_pairs(&target, kind)
                .contains(&("COMMENT".to_string(), "Audit comment".to_string())),
            "{extension}: an empty-description COMM must become COMMENT"
        );
    }
    let target = dir.join("plain.wav");
    encode_with_tags(&audio, &target, OutputFormat::Wav, Some(&tags)).expect("encode WAV");
    assert!(
        wav_id3(&target)
            .iter()
            .any(|frame| matches!(frame, Frame::Comment(comment)
            if comment.description.is_empty() && comment.content == "Audit comment")),
        "the WAV output must keep the empty-description COMM"
    );

    // A described COMM must not merge into COMMENT.
    let described = dir.join("described.mp3");
    write_id3(&described, |tag| {
        tag.insert(Frame::Comment(CommentFrame::new(
            TextEncoding::UTF8,
            *b"eng",
            "Audit",
            "Audit described comment",
        )));
    });
    let tags = read_tags(&described).expect("read the described tags");
    let audio = decode_file(&described).expect("decode");
    for (format, kind, extension) in [
        (OutputFormat::Flac, NativeKind::Flac, "flac"),
        (OutputFormat::Opus, NativeKind::Opus, "opus"),
    ] {
        let target = dir.join(&format!("described.{extension}"));
        let (_, report) = encode_with_tags(&audio, &target, format, Some(&tags)).expect("encode");
        assert!(
            report.contains(&"COMM:Audit".to_string()),
            "{extension}: a described COMM must be named, got {report:?}"
        );
        assert!(
            !native_pairs(&target, kind)
                .iter()
                .any(|(key, _)| key == "COMMENT"),
            "{extension}: a described COMM must not merge into COMMENT"
        );
    }
    let target = dir.join("described.wav");
    encode_with_tags(&audio, &target, OutputFormat::Wav, Some(&tags)).expect("encode WAV");
    assert!(
        wav_id3(&target)
            .iter()
            .any(|frame| matches!(frame, Frame::Comment(comment)
            if comment.description == "Audit" && comment.content == "Audit described comment")),
        "the WAV output must keep the described COMM unchanged"
    );
}

/// The Group B rows, each with the ID3 frame that carries it, the Picard Vorbis key it maps to (or
/// `None` when the Vorbis key is parameterized), and the name the copier reports when it cannot
/// write the field.
struct GroupBRow {
    label: &'static str,
    frame_id: &'static str,
    vorbis: Option<&'static str>,
    reported_as: &'static str,
    value: &'static str,
}

/// The Group B rows of the ruling: the involved-people roles, the performer credits, the rating,
/// and the fingerprint.
const GROUP_B_ROWS: &[GroupBRow] = &[
    GroupBRow {
        label: "Producer",
        frame_id: "TIPL",
        vorbis: Some("PRODUCER"),
        reported_as: "PRODUCER",
        value: "Audit Producer",
    },
    GroupBRow {
        label: "Arranger",
        frame_id: "TIPL",
        vorbis: Some("ARRANGER"),
        reported_as: "ARRANGER",
        value: "Audit Arranger",
    },
    GroupBRow {
        label: "Engineer",
        frame_id: "TIPL",
        vorbis: Some("ENGINEER"),
        reported_as: "ENGINEER",
        value: "Audit Engineer",
    },
    GroupBRow {
        label: "Mix-DJ",
        frame_id: "TIPL",
        vorbis: Some("DJMIXER"),
        reported_as: "DJMIXER",
        value: "Audit Djmixer",
    },
    GroupBRow {
        label: "Mixer",
        frame_id: "TIPL",
        vorbis: Some("MIXER"),
        reported_as: "MIXER",
        value: "Audit Mixer",
    },
    GroupBRow {
        label: "Performer",
        frame_id: "TMCL",
        vorbis: None,
        reported_as: "TMCL",
        value: "Audit Performer",
    },
    GroupBRow {
        label: "Rating",
        frame_id: "POPM",
        vorbis: None,
        reported_as: "POPM",
        value: "Audit Rating",
    },
    GroupBRow {
        label: "Fingerprint",
        frame_id: "TXXX",
        vorbis: None,
        reported_as: "MusicMagic Fingerprint",
        value: "Audit Fingerprint",
    },
];

/// The role each `TIPL` frame carries for the Group B rows.
fn group_b_frame(row: &GroupBRow) -> Frame<'static> {
    match row.label {
        "Producer" | "Arranger" | "Engineer" | "Mix-DJ" | "Mixer" => {
            let role = match row.label {
                "Producer" => "producer",
                "Arranger" => "arranger",
                "Engineer" => "engineer",
                "Mix-DJ" => "DJ-mix",
                _ => "mix",
            };
            Frame::KeyValue(KeyValueFrame::new(
                FrameId::Valid("TIPL".into()),
                TextEncoding::UTF8,
                vec![(
                    std::borrow::Cow::Borrowed(role),
                    std::borrow::Cow::Owned(row.value.to_string()),
                )],
            ))
        }
        "Performer" => Frame::KeyValue(KeyValueFrame::new(
            FrameId::Valid("TMCL".into()),
            TextEncoding::UTF8,
            vec![(
                std::borrow::Cow::Borrowed("instrument"),
                std::borrow::Cow::Owned(row.value.to_string()),
            )],
        )),
        "Rating" => Frame::Popularimeter(PopularimeterFrame::new("user@example.com", 196, 0)),
        _ => Frame::UserText(lofty::id3::v2::ExtendedTextFrame::new(
            TextEncoding::UTF8,
            "MusicMagic Fingerprint".to_string(),
            FRAME_VALUE.to_string(),
        )),
    }
}

/// Group B, ruling 1-3: for each role, performer, rating, and fingerprint row, the field is either
/// preserved exactly under its Picard key in a Vorbis target, or named in the returned list, and a
/// same-format WAV copy keeps the native frame unchanged.
#[test]
fn picard_group_b_rows_are_preserved_or_reported() {
    for row in GROUP_B_ROWS {
        let dir = setup::TempDir::new("melomae-picard-groupb");
        let source = dir.join("s.mp3");
        let frame = group_b_frame(row);
        let native_id = frame.id_str().to_string();
        assert_eq!(native_id, row.frame_id, "{}: the fixture frame", row.label);
        write_id3(&source, |tag| {
            tag.insert(frame);
        });
        let tags = read_tags(&source).expect("read the source tags");
        let audio = decode_file(&source).expect("decode the source");

        for (format, kind, extension) in [
            (OutputFormat::Flac, NativeKind::Flac, "flac"),
            (OutputFormat::Opus, NativeKind::Opus, "opus"),
        ] {
            let target = dir.join(&format!("{}.{extension}", row.label));
            let (_, report) =
                encode_with_tags(&audio, &target, format, Some(&tags)).expect("encode");
            let pairs = native_pairs(&target, kind);
            let preserved = row
                .vorbis
                .is_some_and(|key| pairs.contains(&(key.to_string(), row.value.to_string())));
            // A reported field must not appear under any other key, so an identity change is never
            // silent.
            let reported = report.iter().any(|name| name == row.reported_as);
            assert!(
                preserved || reported,
                "{} -> {extension}: the field must be preserved under its Picard key or named, got pairs={pairs:?} report={report:?}",
                row.label
            );
            if !preserved {
                assert!(
                    pairs.iter().all(|(key, _)| {
                        !key.eq_ignore_ascii_case(row.vorbis.unwrap_or(""))
                    }),
                    "{} -> {extension}: a reported field must not appear under another key, got {pairs:?}",
                    row.label
                );
            }
        }

        let target = dir.join(&format!("{}.wav", row.label));
        encode_with_tags(&audio, &target, OutputFormat::Wav, Some(&tags)).expect("encode WAV");
        if native_id == "TXXX" {
            assert!(
                wav_id3(&target).iter().any(|frame| matches!(frame, Frame::UserText(text)
                    if text.description == "MusicMagic Fingerprint" && text.content == FRAME_VALUE)),
                "{}: the WAV output must keep the native frame unchanged",
                row.label
            );
        } else {
            assert!(
                frame_ids(&wav_id3(&target))
                    .iter()
                    .any(|id| id == &native_id),
                "{}: the WAV output must keep the {native_id} frame unchanged",
                row.label
            );
        }
    }
}

/// Whether a WAV output holds a `USLT` frame with the given text.
fn wav_has_unsynchronized_text(path: &Path, text: &str) -> bool {
    let mut file = std::fs::File::open(path).expect("open the WAV");
    let wav =
        lofty::iff::wav::WavFile::read_from(&mut file, ParseOptions::new()).expect("read WAV");
    wav.id3v2()
        .map(|tag| {
            tag.iter().any(|frame| {
                matches!(frame, Frame::UnsynchronizedText(unsync) if unsync.content == text)
            })
        })
        .unwrap_or(false)
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

/// Add frames to an existing MP3's ID3v2 tag, keeping the fixture's other frames. The reviewer's
/// contract test writes its own native source keys rather than fixture keys produced by the copier.
fn append_id3(path: &Path, build: impl FnOnce(&mut Id3v2Tag)) {
    let mut file = std::fs::File::open(path).expect("open the MP3");
    let mut tag = lofty::mpeg::MpegFile::read_from(&mut file, ParseOptions::new())
        .expect("read the MP3")
        .id3v2()
        .cloned()
        .unwrap_or_default();
    build(&mut tag);
    tag.save_to_path(path, WriteOptions::default())
        .expect("write the MP3 tag");
}

/// Append a Vorbis comment to an existing FLAC, keeping the fixture's other comments.
fn append_vorbis(path: &Path, key: &str, value: &str) {
    let mut file = std::fs::File::open(path).expect("open the FLAC");
    let mut comments = lofty::flac::FlacFile::read_from(&mut file, ParseOptions::new())
        .expect("read the FLAC")
        .vorbis_comments()
        .cloned()
        .unwrap_or_default();
    comments.push(key.to_string(), value.to_string());
    comments
        .save_to_path(path, WriteOptions::default())
        .expect("write the FLAC comments");
}

/// A UTF-8 ID3v2 text frame. A value with a null separator becomes a v2.4 multi-value frame.
fn text_frame(id: &str, value: &str) -> Frame<'static> {
    Frame::Text(TextInformationFrame::new(
        FrameId::Valid(id.to_string().into()),
        TextEncoding::UTF8,
        value.to_string(),
    ))
}

// --- Fix round 5: the reviewer's seven probes over repeated values ----------------------------

/// The values a native reader reports for one field, one string per value.
///
/// A Vorbis target is read by its documented key. An ID3 target (`MP3` or `WAV`) is read by the
/// native frame id: a `TXXX` frame by `TXXX:<description>`, any other frame by its id. A URL frame
/// holds no text pair, so the `WCOP` and `WOAR` frames are read back directly. ID3v2.4 stores a
/// multi-value text frame null-separated in one frame, so the value is split here.
fn native_field_values(path: &Path, kind: NativeKind, key: &str, id3_key: &str) -> Vec<String> {
    let native = if kind == NativeKind::Wav || kind == NativeKind::Mp3 {
        id3_key
    } else {
        key
    };
    let mut values: Vec<String> = native_pairs(path, kind)
        .into_iter()
        .filter(|(candidate, _)| candidate.eq_ignore_ascii_case(native))
        .flat_map(|(_, value)| value.split('\0').map(str::to_string).collect::<Vec<_>>())
        .collect();
    if (kind == NativeKind::Wav || kind == NativeKind::Mp3)
        && (id3_key == "WCOP" || id3_key == "WOAR")
    {
        values.extend(raw_id3_urls(path, id3_key));
    }
    values.sort();
    values
}

/// The URLs of every ID3v2 URL frame with `id` in a file, read from the raw bytes.
///
/// `lofty`'s reader collapses repeated `WOAR`/`WCOP` frames into one, so the raw tag is read here.
/// The tag starts the file for an MP3; a WAV keeps it in the `id3 ` RIFF chunk.
fn raw_id3_urls(path: &Path, id: &str) -> Vec<String> {
    let bytes = std::fs::read(path).expect("read the file");
    let Some(region) = id3_region(&bytes) else {
        return Vec::new();
    };
    if region.len() < 10 {
        return Vec::new();
    }
    let major = region[3];
    let syncsafe = |window: &[u8]| -> usize {
        window.iter().fold(0usize, |total, byte| {
            (total << 7) | (usize::from(*byte) & 0x7f)
        })
    };
    let end = 10 + syncsafe(&region[6..10]);
    let mut offset = 10;
    let mut urls = Vec::new();
    while offset + 10 <= end && end <= region.len() {
        let frame_id = &region[offset..offset + 4];
        if frame_id == [0, 0, 0, 0] {
            break;
        }
        let size = if major >= 4 {
            syncsafe(&region[offset + 4..offset + 8])
        } else {
            (usize::from(region[offset + 4]) << 24)
                | (usize::from(region[offset + 5]) << 16)
                | (usize::from(region[offset + 6]) << 8)
                | usize::from(region[offset + 7])
        };
        let start = offset + 10;
        let stop = start + size;
        if stop > end || stop > region.len() {
            break;
        }
        if frame_id == id.as_bytes() {
            urls.push(String::from_utf8_lossy(&region[start..stop]).into_owned());
        }
        offset = stop;
    }
    urls
}

/// The bytes of the ID3v2 tag in a file: at the start for an MP3, or in the `id3 ` chunk for a WAV.
fn id3_region(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.starts_with(b"ID3") {
        if bytes.len() < 10 {
            return None;
        }
        let size = 10
            + bytes[6..10]
                .iter()
                .fold(0usize, |n, b| (n << 7) | (usize::from(*b) & 0x7f));
        return Some(bytes[..size.min(bytes.len())].to_vec());
    }
    if bytes.starts_with(b"RIFF") {
        let mut offset = 12;
        while offset + 8 <= bytes.len() {
            let id = &bytes[offset..offset + 4];
            let size = u32::from_le_bytes([
                bytes[offset + 4],
                bytes[offset + 5],
                bytes[offset + 6],
                bytes[offset + 7],
            ]) as usize;
            let start = offset + 8;
            let stop = start + size;
            if stop > bytes.len() {
                break;
            }
            if id == b"id3 " || id == b"ID3 " {
                return Some(bytes[start..stop].to_vec());
            }
            offset = stop + (size & 1);
        }
    }
    None
}

/// The license values a native reader reports, from both the `TXXX:LICENSE` text frame and, for an
/// ID3 target, the `WCOP` URL frame.
fn native_license_values_of(path: &Path, kind: NativeKind) -> Vec<String> {
    let mut values = native_field_values(path, kind, "LICENSE", "TXXX:LICENSE");
    if kind == NativeKind::Mp3 || kind == NativeKind::Wav {
        values.extend(native_field_values(path, kind, "LICENSE", "WCOP"));
    }
    values.sort();
    values
}

/// The `(key, wav-frame)` rows the reviewer's probes use, with the exact values each holds.
///
/// Finding 1: the standard override fields Picard stores in a `TXXX` frame. Finding 2: the record
/// label. Finding 5: the scalar text lists that share the same generic conversion.
const OVERRIDE_TEXT_ROWS: [(&str, &str); 3] = [
    ("ARTISTS", "TXXX:Artists"),
    ("WRITER", "TXXX:Writer"),
    ("WORK", "TXXX:WORK"),
];

/// Finding 1 (blocking): every override text value survives in all three outputs. A native FLAC
/// source holds two values for `ARTISTS`, `WRITER`, and `WORK`; each Vorbis output keeps both, and
/// the WAV `TXXX` frame is checked for both values too.
#[test]
fn override_text_values_all_survive_in_every_output() {
    let fixtures = TagFixtures::build();
    let source = fixtures.path(Source::Flac);
    for (key, _) in OVERRIDE_TEXT_ROWS {
        for value in ["Alice", "Bob"] {
            append_vorbis(&source, key, value);
        }
    }
    let tags = read_tags(&source).expect("read the source tags");
    let audio = decode_file(&source).expect("decode the source");
    let outputs = setup::TempDir::new("melomae-override-text");

    let mut failures = Vec::new();
    for (format, kind, extension) in OUTPUTS {
        let target = outputs.join(&format!("override.{extension}"));
        let (_, report) = encode_with_tags(&audio, &target, format, Some(&tags))
            .unwrap_or_else(|error| panic!("-> {extension}: {error}"));
        for (key, id3_key) in OVERRIDE_TEXT_ROWS {
            let want = native_field_values(&source, NativeKind::Flac, key, id3_key);
            let got = native_field_values(&target, kind, key, id3_key);
            for value in &want {
                if !got.contains(value) {
                    failures.push(format!(
                        "{key} -> {format:?}: lost {value:?}, got {got:?} report={report:?}"
                    ));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Findings 2 and 5: the record label (blocking) and the scalar text lists. Every value of the
/// label must survive in every output. A scalar text list must either survive in full or be named
/// in the returned list, so no value is dropped in silence.
#[test]
fn generic_text_values_survive_or_are_reported() {
    let fixtures = TagFixtures::build();
    let source = fixtures.path(Source::Flac);
    let rows = [
        ("LABEL", "TPUB"),
        ("ARTISTSORT", "TSOP"),
        ("ALBUMARTISTSORT", "TSO2"),
        ("COMPOSERSORT", "TSOC"),
        ("ENCODERSETTINGS", "TSSE"),
    ];
    for (key, _) in rows {
        for value in ["First", "Second"] {
            append_vorbis(&source, key, value);
        }
    }
    let tags = read_tags(&source).expect("read the source tags");
    let audio = decode_file(&source).expect("decode the source");
    let outputs = setup::TempDir::new("melomae-generic-text");

    let mut failures = Vec::new();
    for (format, kind, extension) in OUTPUTS {
        let target = outputs.join(&format!("generic.{extension}"));
        let (_, report) = encode_with_tags(&audio, &target, format, Some(&tags))
            .unwrap_or_else(|error| panic!("-> {extension}: {error}"));
        for (key, id3_key) in rows {
            let want = native_field_values(&source, NativeKind::Flac, key, id3_key);
            let got = native_field_values(&target, kind, key, id3_key);
            if key == "LABEL" {
                for value in &want {
                    if !got.contains(value) {
                        failures.push(format!(
                            "{key} -> {format:?}: lost {value:?}, got {got:?} report={report:?}"
                        ));
                    }
                }
            } else if !report.iter().any(|name| name == key) && want != got {
                failures.push(format!(
                    "{key} -> {format:?}: want={want:?} got={got:?} report={report:?}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Finding 3 (blocking): every WEBSITE URL survives in FLAC and Opus. `lofty`'s `Id3v2Tag` cannot
/// hold two frames with the same id, so the WAV output keeps the first URL in its `WOAR` frame and
/// names `WEBSITE` in the returned list for the rest.
#[test]
fn website_values_all_survive_in_every_output() {
    let fixtures = TagFixtures::build();
    let source = fixtures.path(Source::Flac);
    let want = ["https://example.com/a", "https://example.com/b"];
    for value in want {
        append_vorbis(&source, "WEBSITE", value);
    }
    let mut expected: Vec<String> = want.iter().map(|value| (*value).to_string()).collect();
    expected.sort();
    assert_eq!(
        native_field_values(&source, NativeKind::Flac, "WEBSITE", "WOAR"),
        expected,
        "the source must hold both websites"
    );

    let tags = read_tags(&source).expect("read the source tags");
    let audio = decode_file(&source).expect("decode the source");
    let outputs = setup::TempDir::new("melomae-website");

    let mut failures = Vec::new();
    for (format, kind, extension) in OUTPUTS {
        let target = outputs.join(&format!("website.{extension}"));
        let (_, report) = encode_with_tags(&audio, &target, format, Some(&tags))
            .unwrap_or_else(|error| panic!("-> {extension}: {error}"));
        if format == OutputFormat::Wav {
            let urls = raw_id3_urls(&target, "WOAR");
            if urls != vec![want[0].to_string()] {
                failures.push(format!(
                    "WAV: the first URL must be the one WOAR, got {urls:?} report={report:?}"
                ));
            }
            if !report.iter().any(|name| name == "WEBSITE") {
                failures.push(format!(
                    "WAV: the dropped website must be named, report={report:?}"
                ));
            }
        } else {
            let got = native_field_values(&target, kind, "WEBSITE", "WOAR");
            if got != expected {
                failures.push(format!("{format:?}: got={got:?} report={report:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Finding 3, second path: lofty's reader collapses two `WOAR` frames in an MP3 source into one, so
/// the loss happens before the write. `WEBSITE` must be named in the returned list, or both URLs
/// must survive.
#[test]
fn an_mp3_two_woar_frames_are_reported_or_survive() {
    let dir = setup::TempDir::new("melomae-mp3-woar");
    let source = dir.join("s.mp3");
    write_id3(&source, |_| {});

    // Replace the tag with a raw ID3v2.4 tag that holds two `WOAR` frames. Lofty's reader keeps
    // only the last, so a small independent frame walk is needed to see both.
    let original = std::fs::read(&source).expect("read the MP3");
    let skip = if original.starts_with(b"ID3") {
        10 + original[6..10]
            .iter()
            .fold(0usize, |n, b| (n << 7) | (*b as usize & 0x7f))
    } else {
        0
    };
    let want = ["https://example.com/a", "https://example.com/b"];
    let mut frames = Vec::new();
    for value in want {
        frames.extend_from_slice(b"WOAR");
        frames.extend_from_slice(&[0, 0, 0, value.len() as u8, 0, 0]);
        frames.extend_from_slice(value.as_bytes());
    }
    let len = frames.len();
    let mut bytes = b"ID3\x04\0\0".to_vec();
    bytes.extend_from_slice(&[
        ((len >> 21) & 127) as u8,
        ((len >> 14) & 127) as u8,
        ((len >> 7) & 127) as u8,
        (len & 127) as u8,
    ]);
    bytes.extend(frames);
    bytes.extend_from_slice(&original[skip..]);
    std::fs::write(&source, bytes).expect("write the raw MP3");

    let tags = read_tags(&source).expect("read the source tags");
    let audio = decode_file(&source).expect("decode the source");
    let outputs = setup::TempDir::new("melomae-mp3-woar-out");

    let mut expected: Vec<String> = want.iter().map(|value| (*value).to_string()).collect();
    expected.sort();

    let mut failures = Vec::new();
    for (format, kind, extension) in OUTPUTS {
        let target = outputs.join(&format!("woar.{extension}"));
        let (_, report) = encode_with_tags(&audio, &target, format, Some(&tags))
            .unwrap_or_else(|error| panic!("-> {extension}: {error}"));
        let got = native_field_values(&target, kind, "WEBSITE", "WOAR");
        let reported = report.iter().any(|name| name == "WEBSITE");
        if got != expected && !reported {
            failures.push(format!(
                "{format:?}: want={expected:?} got={got:?} report={report:?}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Finding 4 (hardening): repeated movement numbers and totals. A Vorbis target keeps every value.
/// The WAV `MVIN` frame can express only one pair, so either both pairs survive or the movement
/// field is named in the returned list.
#[test]
fn movement_values_survive_or_are_reported() {
    let fixtures = TagFixtures::build();
    let source = fixtures.path(Source::Flac);
    for value in ["1", "2"] {
        append_vorbis(&source, "MOVEMENT", value);
    }
    for value in ["3", "4"] {
        append_vorbis(&source, "MOVEMENTTOTAL", value);
    }
    let tags = read_tags(&source).expect("read the source tags");
    let audio = decode_file(&source).expect("decode the source");
    let outputs = setup::TempDir::new("melomae-movement");

    let mut failures = Vec::new();
    for (format, kind, extension) in OUTPUTS {
        let target = outputs.join(&format!("movement.{extension}"));
        let (_, report) = encode_with_tags(&audio, &target, format, Some(&tags))
            .unwrap_or_else(|error| panic!("-> {extension}: {error}"));
        let pairs = native_pairs(&target, kind);
        if format == OutputFormat::Wav {
            let both = pairs.contains(&("MVIN".to_string(), "1/3".to_string()))
                && pairs.contains(&("MVIN".to_string(), "2/4".to_string()));
            let reported = report
                .iter()
                .any(|name| name == "MOVEMENT" || name == "MOVEMENTTOTAL");
            if !both && !reported {
                failures.push(format!("WAV: got {pairs:?} report={report:?}"));
            }
        } else {
            for (key, values) in [("MOVEMENT", ["1", "2"]), ("MOVEMENTTOTAL", ["3", "4"])] {
                for value in values {
                    if !pairs.contains(&(key.to_string(), value.to_string())) {
                        failures.push(format!(
                            "{extension}: {key}={value:?} was lost, got {pairs:?}"
                        ));
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Control: the license row from a rear view. Three source families hold the same license values;
/// each value survives in all three outputs, and the WAV frame is `WCOP` for a single URL and one
/// `TXXX:LICENSE` frame otherwise.
#[test]
fn license_matrix_keeps_every_value() {
    let mut failures = Vec::new();
    for values in [
        vec!["https://example.com/a", "https://example.com/b"],
        vec!["https://example.com/a", "All rights reserved"],
        vec!["https://example.com/a"],
        vec!["All rights reserved"],
        vec!["https://example.com/a", "https://example.com/a"],
    ] {
        let fixtures = TagFixtures::build();
        let dir = setup::TempDir::new("melomae-license-matrix");
        let flac = fixtures.path(Source::Flac);
        for value in &values {
            append_vorbis(&flac, "LICENSE", value);
        }
        let mp3 = dir.join("s.mp3");
        write_id3(&mp3, |tag| {
            tag.insert_user_text("LICENSE".to_string(), values.join("\0"));
        });
        let wav = dir.join("s.wav");
        let audio = decode_file(&mp3).expect("decode the MP3");
        encode(&audio, &wav, OutputFormat::Wav).expect("encode the WAV");
        let mut tag = Id3v2Tag::new();
        tag.insert_user_text("LICENSE".to_string(), values.join("\0"));
        tag.save_to_path(&wav, WriteOptions::default())
            .expect("write the WAV tag");

        let mut want: Vec<String> = values.iter().map(|value| (*value).to_string()).collect();
        want.sort();

        for (source, kind) in [
            (&flac, NativeKind::Flac),
            (&mp3, NativeKind::Mp3),
            (&wav, NativeKind::Wav),
        ] {
            let found = native_license_values_of(source, kind);
            assert_eq!(
                found, want,
                "source {kind:?} must hold the literal license values"
            );

            let tags = read_tags(source).expect("read the license source");
            let audio = decode_file(source).expect("decode the license source");
            for (format, out_kind, extension) in OUTPUTS {
                let target = dir.join(&format!("{kind:?}.{extension}"));
                let (_, report) = encode_with_tags(&audio, &target, format, Some(&tags))
                    .unwrap_or_else(|error| panic!("-> {extension}: {error}"));
                let got = native_license_values_of(&target, out_kind);
                if got != want {
                    failures.push(format!(
                        "{kind:?} -> {format:?}: want={want:?} got={got:?} report={report:?}"
                    ));
                }
                if format == OutputFormat::Wav {
                    let native = wav_id3(&target);
                    if values.len() == 1 && values[0].starts_with("https://") {
                        assert!(has_url(&native, "WCOP", values[0]));
                    } else {
                        assert!(native
                            .iter()
                            .any(|frame| matches!(frame, Frame::UserText(text)
                            if text.description == "LICENSE"
                                && text.content == values.join("\0"))));
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Control: a WAV source that holds both a `WCOP` frame and a two-value `TXXX:LICENSE` frame keeps
/// all three values in every output.
#[test]
fn wav_mixed_license_frames_keep_every_value() {
    let dir = setup::TempDir::new("melomae-mixed-license");
    let mp3 = dir.join("s.mp3");
    write_id3(&mp3, |_| {});
    let wav = dir.join("s.wav");
    let audio = decode_file(&mp3).expect("decode the MP3");
    encode(&audio, &wav, OutputFormat::Wav).expect("encode the WAV");
    let mut native = Id3v2Tag::new();
    native.insert(Frame::Url(UrlLinkFrame::new(
        FrameId::Valid("WCOP".into()),
        "https://example.com/a".to_string(),
    )));
    native.insert_user_text(
        "LICENSE".to_string(),
        "https://example.com/b\0All rights reserved".to_string(),
    );
    native
        .save_to_path(&wav, WriteOptions::default())
        .expect("write the WAV tag");

    let want = vec![
        "All rights reserved".to_string(),
        "https://example.com/a".to_string(),
        "https://example.com/b".to_string(),
    ];
    assert_eq!(native_license_values_of(&wav, NativeKind::Wav), want);

    let tags = read_tags(&wav).expect("read the WAV tags");
    for (format, kind, extension) in OUTPUTS {
        let target = dir.join(&format!("out.{extension}"));
        let (_, report) = encode_with_tags(&audio, &target, format, Some(&tags))
            .unwrap_or_else(|error| panic!("-> {extension}: {error}"));
        assert_eq!(
            native_license_values_of(&target, kind),
            want,
            "{format:?} report={report:?}"
        );
    }
}

/// The write check itself: a source whose WAV writer can hold only one movement value must name
/// `MOVEMENT` in the returned list. This is the field the check must catch, even though no
/// dedicated rule covers repeated movement.
#[test]
fn the_write_check_reports_a_value_the_wav_writer_drops() {
    let fixtures = TagFixtures::build();
    let source = fixtures.path(Source::Flac);
    append_vorbis(&source, "MOVEMENT", "1");
    append_vorbis(&source, "MOVEMENT", "2");

    let tags = read_tags(&source).expect("read the source tags");
    let audio = decode_file(&source).expect("decode the source");
    let outputs = setup::TempDir::new("melomae-write-check");
    let target = outputs.join("movement.wav");
    let (_, report) =
        encode_with_tags(&audio, &target, OutputFormat::Wav, Some(&tags)).expect("encode the WAV");
    assert!(
        report.contains(&"MOVEMENT".to_string()),
        "the write check must name the movement field, got {report:?}"
    );
}
