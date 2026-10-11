//! Copy every tag and picture from a source file into each output format.
//!
//! The three source fixtures are built in `setup::tag_fixtures` with the pinned ffmpeg and lofty.
//! Each source copies to FLAC, WAV, and Opus, and every source field and the front cover must
//! survive, or appear in the returned list of fields the target cannot hold.

mod common;
mod setup;

use lofty::picture::PictureType;
use lofty::tag::TagType;

use melomae_core::decode::decode_file;
use melomae_core::encode::{encode_with_tags, OutputFormat};
use melomae_core::tag_copy::{read_tags, TagSet};

use setup::tag_fixtures::{
    Source, TagFixtures, ALBUM, ALBUM_ARTIST, ALBUM_ARTIST_SORT, ARTIST, COMPOSERS,
    COVER_DESCRIPTION, CUSTOM_NAME, CUSTOM_VALUE, SCRIPT, TITLE,
};

/// The portable name of a standard field: its Vorbis key, so the same field compares equal across
/// formats.
///
/// Some fields share one native frame. ID3v2 stores the label and the publisher in `TPUB`, the
/// lyricist and the writer in `TEXT`, and the encoder software and its settings in `TSSE`. Each
/// such pair is one field, so the two lofty names screen to one portable name.
fn standard_name(key: lofty::tag::ItemKey) -> String {
    use lofty::tag::ItemKey;
    match key {
        ItemKey::Publisher | ItemKey::Label => "LABEL".to_string(),
        ItemKey::Writer | ItemKey::Lyricist => "LYRICIST".to_string(),
        ItemKey::EncoderSettings | ItemKey::EncoderSoftware => "ENCODER".to_string(),
        other => other
            .map_key(TagType::VorbisComments)
            .map(str::to_string)
            .unwrap_or_else(|| format!("{other:?}")),
    }
}

/// A source field as a portable `(name, value)` pair, or the custom name.
fn portable(tags: &TagSet) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for item in tags.standard() {
        let name = standard_name(item.key());
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

/// The output formats, with a name for a file.
const OUTPUTS: [(OutputFormat, &str); 3] = [
    (OutputFormat::Flac, "flac"),
    (OutputFormat::Wav, "wav"),
    (OutputFormat::Opus, "opus"),
];

/// Test-first plan item 2: for each source family, every field and the front cover survives in all
/// three output formats, or the field is named in the list the target cannot hold.
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
        assert!(
            !tags.standard().is_empty(),
            "{family}: the fixture holds standard fields"
        );
        assert!(
            !tags.custom().is_empty(),
            "{family}: the fixture holds custom fields"
        );

        let audio = decode_file(&path).unwrap_or_else(|error| panic!("decode {family}: {error}"));
        for (format, extension) in OUTPUTS {
            let target = outputs.join(&format!("{family}.{extension}"));
            let (_, unsupported) = encode_with_tags(&audio, &target, format, Some(&tags))
                .unwrap_or_else(|error| panic!("{family} -> {extension}: {error}"));

            // The tagged output must still decode.
            decode_file(&target)
                .unwrap_or_else(|error| panic!("decode {family}.{extension}: {error}"));

            let written = read_tags(&target)
                .unwrap_or_else(|error| panic!("read {family}.{extension}: {error}"));
            let present = portable(&written);
            for (name, value) in portable(&tags) {
                if !present.contains(&(name.clone(), value.clone())) {
                    assert!(
                        unsupported.contains(&name),
                        "{family} -> {extension}: {name}={value:?} was dropped without a report"
                    );
                }
            }

            let cover = &written.pictures()[0];
            assert_eq!(
                cover.data(),
                tags.pictures()[0].data(),
                "{family} -> {extension}: the cover bytes must survive"
            );
            assert_eq!(
                cover.mime_type(),
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

        for (format, extension) in OUTPUTS {
            let target = outputs.join(&format!("{family}-mapped.{extension}"));
            let (_, _) = encode_with_tags(&audio, &target, format, Some(&tags)).expect("encode");

            let written = read_tags(&target).expect("read the output tags");
            let pairs = portable(&written);

            // The album artist crosses ID3 `TPE2`, MP4 `aART`, and Vorbis `ALBUMARTIST` as one
            // field: the Vorbis key is the portable name.
            assert!(
                pairs.contains(&("ALBUMARTIST".to_string(), ALBUM_ARTIST.to_string())),
                "{family} -> {extension}: the album artist did not survive as ALBUMARTIST"
            );
            // The album artist sort order crosses `TSO2`, `soaa`, and `ALBUMARTISTSORT`.
            assert!(
                pairs.contains(&("ALBUMARTISTSORT".to_string(), ALBUM_ARTIST_SORT.to_string())),
                "{family} -> {extension}: the album artist sort order did not survive"
            );
            // A custom field crosses every format by its own name.
            assert!(
                pairs.contains(&(CUSTOM_NAME.to_string(), CUSTOM_VALUE.to_string())),
                "{family} -> {extension}: the custom field did not survive"
            );
            // A multi-value field keeps every value.
            let composers: Vec<&String> = pairs
                .iter()
                .filter(|(name, _)| name == "COMPOSER")
                .map(|(_, value)| value)
                .collect();
            for composer in COMPOSERS {
                assert!(
                    composers.contains(&&composer.to_string()),
                    "{family} -> {extension}: composer {composer:?} was lost, kept {composers:?}"
                );
            }

            if source == Source::M4a {
                // MP4 holds no picture type, so its front cover reads back as a plain picture.
                assert_eq!(
                    written.pictures()[0].pic_type(),
                    PictureType::Other,
                    "{family} -> {extension}: MP4 cannot store a picture type"
                );
            } else {
                assert_eq!(
                    written.pictures()[0].pic_type(),
                    PictureType::CoverFront,
                    "{family} -> {extension}: a front cover must stay a front cover"
                );
                assert_eq!(
                    written.pictures()[0].description(),
                    Some(COVER_DESCRIPTION),
                    "{family} -> {extension}: the cover description must survive"
                );
            }
        }
    }
}

/// Scope item 7: a field the target cannot hold is named in the returned list. ID3v2 has no frame
/// for the script field, so a WAV output reports it.
#[test]
fn e2e_tag_copy_reports_a_field_the_target_cannot_hold() {
    let fixtures = TagFixtures::build();
    let outputs = setup::TempDir::new("melomae-tag-unsupported");

    // The FLAC fixture carries the script as the standard `SCRIPT` field.
    let path = fixtures.path(Source::Flac);
    let tags = read_tags(&path).expect("read the source tags");
    assert!(
        portable(&tags).contains(&("SCRIPT".to_string(), SCRIPT.to_string())),
        "the FLAC fixture must hold the script field"
    );
    let audio = decode_file(&path).expect("decode the source");

    let target = outputs.join("script.wav");
    let (_, unsupported) =
        encode_with_tags(&audio, &target, OutputFormat::Wav, Some(&tags)).expect("encode WAV");
    assert!(
        unsupported.contains(&"SCRIPT".to_string()),
        "the script field must be reported, got {unsupported:?}"
    );

    let written = read_tags(&target).expect("read the WAV tags");
    assert!(
        !portable(&written).contains(&("SCRIPT".to_string(), SCRIPT.to_string())),
        "a field the target cannot hold must not appear"
    );

    // The same field survives in FLAC and Opus, which do hold it.
    for (format, extension) in [(OutputFormat::Flac, "flac"), (OutputFormat::Opus, "opus")] {
        let target = outputs.join(&format!("script.{extension}"));
        let (_, unsupported) =
            encode_with_tags(&audio, &target, format, Some(&tags)).expect("encode");
        assert!(
            !unsupported.contains(&"SCRIPT".to_string()),
            "{extension}: the script field is supported"
        );
        let written = read_tags(&target).expect("read the tags");
        assert!(
            portable(&written).contains(&("SCRIPT".to_string(), SCRIPT.to_string())),
            "{extension}: the script field must survive"
        );
    }
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
        for (format, extension) in OUTPUTS {
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
            for needle in [TITLE, ARTIST, ALBUM, CUSTOM_VALUE] {
                assert!(
                    dump.contains(needle),
                    "{family} -> {extension}: ffprobe did not report {needle:?}\n{dump}"
                );
            }
        }
    }
}
