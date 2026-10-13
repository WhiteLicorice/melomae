//! Copy every tag and picture from a source file to an output file.
//!
//! [`read_tags`] reads a file's tags with `lofty`, carries the standard fields in lofty's generic
//! [`Tag`] model, and keeps the fields that the generic layer cannot hold (ID3 `TXXX`, Vorbis keys
//! with no [`ItemKey`], MP4 freeform atoms) as named custom fields. [`write_into`] writes a
//! [`TagSet`] into an output file in the output's own tag format: ID3 for WAV, Vorbis comments for
//! FLAC and Opus.
//!
//! The standard-field mapping starts from lofty's generic mapping, and an explicit override table
//! corrects the rows where lofty's key differs from the MusicBrainz Picard tag mapping. A field that
//! the target format cannot hold is never dropped in silence: [`TagSet::unsupported_for`] names it.
//! [`write_losses`] compares the source with a read-back of the written file, so a value a writer
//! drops is named too, even when no rule predicted the loss.

use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::path::{Path, PathBuf};

use lofty::config::{ParseOptions, WriteOptions};
use lofty::file::{AudioFile, FileType, TaggedFileExt};
use lofty::id3::v2::{ExtendedTextFrame, Frame, FrameId, TextInformationFrame, UrlLinkFrame};
use lofty::mp4::{AtomData, AtomIdent};
use lofty::ogg::tag::VorbisComments;
use lofty::picture::Picture;
use lofty::tag::{ItemKey, ItemValue, Tag, TagExt, TagItem, TagType};
use lofty::TextEncoding;

use crate::encode::OutputFormat;

/// A field kept by its own name: an ID3 `TXXX`, a Vorbis key with no standard equivalent, or an
/// MP4 freeform atom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomField {
    name: String,
    values: Vec<String>,
}

impl CustomField {
    /// The field name. An ID3 `TXXX:NAME` and an MP4 freeform atom `NAME` both record `NAME`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Every value of the field, in order. A multi-value field keeps each value.
    pub fn values(&self) -> &[String] {
        &self.values
    }
}

/// A native ID3v2 frame the copier carries verbatim into a WAV output, because the generic model
/// cannot represent it. `name` is how the returned list names it for a Vorbis target, where the
/// frame has no plain equivalent.
#[derive(Debug, Clone)]
struct CarriedFrame {
    name: String,
    frame: Frame<'static>,
}

/// Every tag and picture read from one file.
#[derive(Debug, Clone, Default)]
pub struct TagSet {
    standard: Vec<TagItem>,
    custom: Vec<CustomField>,
    pictures: Vec<Picture>,
    carried: Vec<CarriedFrame>,
    /// Fields the source reader itself lost, so the report names them even though the tag set no
    /// longer holds a value to compare. `lofty` can collapse several valid frames with the same id
    /// into one; the id is recorded here.
    lost: Vec<String>,
}

impl TagSet {
    /// The standard fields, in lofty's generic model. A multi-value field is one entry per value.
    pub fn standard(&self) -> &[TagItem] {
        &self.standard
    }

    /// The custom fields, in order.
    pub fn custom(&self) -> &[CustomField] {
        &self.custom
    }

    /// Every embedded picture, with its type, description, and MIME type.
    pub fn pictures(&self) -> &[Picture] {
        &self.pictures
    }

    /// The fields that the target format cannot hold. The job report lists these, so no field is
    /// dropped in silence. A carried ID3v2 frame is preserved in a WAV output, and named for a
    /// Vorbis target that has no plain equivalent. A field the source reader already lost is named
    /// for every target.
    pub fn unsupported_for(&self, format: OutputFormat) -> Vec<String> {
        let tag_type = target_tag_type(format);
        let mut names = self.lost.clone();
        for item in &self.standard {
            if !standard_is_supported(item, tag_type) {
                names.push(standard_name(item.key()));
            }
        }
        for field in &self.custom {
            if !custom_is_supported(field, format) {
                names.push(field.name.clone());
            }
        }
        if format != OutputFormat::Wav {
            for carried in &self.carried {
                names.push(carried.name.clone());
            }
        }
        names
    }
}

/// An error from reading or writing tags.
///
/// The error always names the file and states the reason, so a caller can report both.
#[derive(Debug)]
pub struct TagError {
    path: PathBuf,
    reason: String,
}

impl TagError {
    fn new(path: &Path, reason: impl Into<String>) -> Self {
        Self {
            path: path.to_path_buf(),
            reason: reason.into(),
        }
    }

    /// The file that could not be read or written.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Why the file could not be read or written.
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl fmt::Display for TagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.reason)
    }
}

impl std::error::Error for TagError {}

/// The tag type a WAV, FLAC, or Opus output carries.
fn target_tag_type(format: OutputFormat) -> TagType {
    match format {
        OutputFormat::Wav => TagType::Id3v2,
        OutputFormat::Flac | OutputFormat::Opus => TagType::VorbisComments,
    }
}

/// A field whose Picard name differs from lofty's generic mapping. The portable name is the
/// Vorbis key. `key` is the lofty [`ItemKey`] the row overrides when the field arrives as a
/// standard item, so the reader and writer use the Picard name instead of lofty's. `id3` is the
/// ID3v2 name, either a frame id or, when `id3_user_text` is set, a `TXXX` description. `mp4` is
/// the MP4 freeform atom name, without the `----:com.apple.iTunes:` prefix.
struct PicardMapping {
    key: Option<ItemKey>,
    portable: &'static str,
    id3: Option<&'static str>,
    id3_user_text: bool,
    mp4: Option<&'static str>,
}

/// The fields the MusicBrainz Picard tag mapping names differently from lofty.
///
/// Each row holds the Picard Vorbis key as the portable name. `key` is the lofty [`ItemKey`] the
/// row overrides when the field arrives as a standard item, so the reader and writer use the Picard
/// name instead of lofty's. `id3` is the ID3v2 frame id, or, when `id3_user_text` is set, the `TXXX`
/// description. `mp4` is the iTunes MP4 name without the `----:com.apple.iTunes:` prefix.
///
/// `LABEL`: Picard maps ID3 `TPUB` to Vorbis `LABEL`, but lofty reads `TPUB` as `Publisher` and
/// writes Vorbis `PUBLISHER`. `LYRICS`: Picard maps ID3 `USLT` to Vorbis `LYRICS`, but lofty names
/// `UnsyncLyrics` `UNSYNCEDLYRICS`. `KEY`: Picard's Vorbis key is `KEY`, but lofty prefers
/// `INITIALKEY`. `BPM`: Picard maps ID3 `TBPM` to Vorbis `BPM`, but lofty's `IntegerBpm` has no
/// Vorbis key. `COMPOSERSORT`: Picard maps ID3 `TSOC` to Vorbis `COMPOSERSORT`, but lofty's
/// `ComposerSortOrder` has no Vorbis key. `WRITER`: Picard maps ID3 `TXXX:Writer` to Vorbis
/// `WRITER`, but lofty's Vorbis key for `Writer` is `AUTHOR`. `ARTISTS`: Picard's ID3 name is
/// `TXXX:Artists`, but lofty writes `TXXX:ARTISTS`. `WORK`: Picard maps ID3 `TXXX:WORK`
/// to Vorbis `WORK`, but lofty writes a bare `WORK` text frame, which is invalid. `TOFN`:
/// `OriginalFileName` has no Vorbis key in lofty. `SCRIPT` and `RELEASESTATUS`: no ID3 `ItemKey` in
/// lofty, so the generic layer reports or mis-names them. `ORIGINALYEAR`: shares lofty's
/// `OriginalReleaseDate` with `ORIGINALDATE`, and Picard defines no ID3 frame for it.
/// `ENCODERSETTINGS`: Picard's Vorbis key is `ENCODERSETTINGS`, but lofty writes `ENCODING`.
/// `MUSICBRAINZ_COMPOSERID`, `MUSICBRAINZ_DISCID`, `MUSICBRAINZ_ORIGINALARTISTID`,
/// `MUSICBRAINZ_ORIGINALALBUMID`, `MUSICBRAINZ_TRMID`, `MUSICIP_PUID`: lofty has no mapping, so the
/// generic layer passes the spaced ID3 description straight through as the Vorbis key.
///
/// Source: <https://picard-docs.musicbrainz.org/en/latest/_static/MusicBrainz_Picard_Tag_Map.html>,
/// checked 2026-10-13.
const PICARD_MAPPINGS: &[PicardMapping] = &[
    PicardMapping {
        key: None,
        portable: "LABEL",
        id3: Some("TPUB"),
        id3_user_text: false,
        mp4: Some("LABEL"),
    },
    PicardMapping {
        key: Some(ItemKey::TrackArtistUrl),
        portable: "WEBSITE",
        id3: Some("WOAR"),
        id3_user_text: false,
        mp4: None,
    },
    PicardMapping {
        key: Some(ItemKey::CopyrightUrl),
        portable: "LICENSE",
        id3: Some("WCOP"),
        id3_user_text: false,
        mp4: None,
    },
    PicardMapping {
        key: Some(ItemKey::TrackArtists),
        portable: "ARTISTS",
        id3: Some("Artists"),
        id3_user_text: true,
        mp4: Some("ARTISTS"),
    },
    PicardMapping {
        key: Some(ItemKey::Script),
        portable: "SCRIPT",
        id3: Some("SCRIPT"),
        id3_user_text: true,
        mp4: Some("SCRIPT"),
    },
    PicardMapping {
        key: None,
        portable: "RELEASESTATUS",
        id3: Some("MusicBrainz Album Status"),
        id3_user_text: true,
        mp4: Some("MusicBrainz Album Status"),
    },
    PicardMapping {
        key: None,
        portable: "ORIGINALYEAR",
        id3: None,
        id3_user_text: false,
        mp4: None,
    },
    PicardMapping {
        key: Some(ItemKey::EncoderSettings),
        portable: "ENCODERSETTINGS",
        id3: Some("TSSE"),
        id3_user_text: false,
        mp4: None,
    },
    PicardMapping {
        key: Some(ItemKey::UnsyncLyrics),
        portable: "LYRICS",
        id3: Some("USLT"),
        id3_user_text: false,
        mp4: Some("©lyr"),
    },
    PicardMapping {
        key: Some(ItemKey::InitialKey),
        portable: "KEY",
        id3: Some("TKEY"),
        id3_user_text: false,
        mp4: Some("initialkey"),
    },
    PicardMapping {
        key: Some(ItemKey::IntegerBpm),
        portable: "BPM",
        id3: Some("TBPM"),
        id3_user_text: false,
        mp4: Some("tmpo"),
    },
    PicardMapping {
        key: Some(ItemKey::ComposerSortOrder),
        portable: "COMPOSERSORT",
        id3: Some("TSOC"),
        id3_user_text: false,
        mp4: Some("soco"),
    },
    PicardMapping {
        key: Some(ItemKey::Writer),
        portable: "WRITER",
        id3: Some("Writer"),
        id3_user_text: true,
        mp4: None,
    },
    PicardMapping {
        key: Some(ItemKey::Work),
        portable: "WORK",
        id3: Some("WORK"),
        id3_user_text: true,
        mp4: Some("©wrk"),
    },
    PicardMapping {
        key: Some(ItemKey::OriginalFileName),
        portable: "ORIGINALFILENAME",
        id3: Some("TOFN"),
        id3_user_text: false,
        mp4: None,
    },
    PicardMapping {
        key: None,
        portable: "MUSICBRAINZ_COMPOSERID",
        id3: Some("MusicBrainz Composer Id"),
        id3_user_text: true,
        mp4: Some("MusicBrainz Composer Id"),
    },
    PicardMapping {
        key: None,
        portable: "MUSICBRAINZ_DISCID",
        id3: Some("MusicBrainz Disc Id"),
        id3_user_text: true,
        mp4: Some("MusicBrainz Disc Id"),
    },
    PicardMapping {
        key: None,
        portable: "MUSICBRAINZ_ORIGINALARTISTID",
        id3: Some("MusicBrainz Original Artist Id"),
        id3_user_text: true,
        mp4: Some("MusicBrainz Original Artist Id"),
    },
    PicardMapping {
        key: None,
        portable: "MUSICBRAINZ_ORIGINALALBUMID",
        id3: Some("MusicBrainz Original Album Id"),
        id3_user_text: true,
        mp4: Some("MusicBrainz Original Album Id"),
    },
    PicardMapping {
        key: None,
        portable: "MUSICBRAINZ_TRMID",
        id3: Some("MusicBrainz TRM Id"),
        id3_user_text: true,
        mp4: Some("MusicBrainz TRM Id"),
    },
    PicardMapping {
        key: None,
        portable: "MUSICIP_PUID",
        id3: Some("MusicIP PUID"),
        id3_user_text: true,
        mp4: Some("MusicIP PUID"),
    },
];

/// The mapping for a portable (Vorbis) name.
fn picard_mapping(portable: &str) -> Option<&'static PicardMapping> {
    PICARD_MAPPINGS.iter().find(|m| m.portable == portable)
}

/// The mapping that overrides a standard [`ItemKey`], so its Picard name wins over lofty's.
///
/// Picard's license row is reached through two lofty keys: an ID3 `WCOP` frame reads as
/// `CopyrightUrl`, and a Vorbis `LICENSE` comment reads as `License`. Both take the license row.
fn picard_mapping_for_key(key: ItemKey) -> Option<&'static PicardMapping> {
    if let Some(mapping) = PICARD_MAPPINGS.iter().find(|m| m.key == Some(key)) {
        return Some(mapping);
    }
    if key == ItemKey::License {
        return picard_mapping("LICENSE");
    }
    None
}

/// The mapping for an ID3 `TXXX` description. The comparison ignores case, spaces, and
/// underscores, so the documented name matches whether it carries its spaces or not.
fn picard_mapping_for_id3_text(description: &str) -> Option<&'static PicardMapping> {
    PICARD_MAPPINGS
        .iter()
        .find(|m| m.id3_user_text && m.id3.is_some_and(|name| same_field_name(name, description)))
}

/// The mapping for an MP4 freeform atom name.
fn picard_mapping_for_mp4(name: &str) -> Option<&'static PicardMapping> {
    PICARD_MAPPINGS
        .iter()
        .find(|m| m.mp4.is_some_and(|native| same_field_name(native, name)))
}

/// Whether two field names are the same, ignoring case, spaces, and underscores.
fn same_field_name(a: &str, b: &str) -> bool {
    let normalize = |name: &str| -> String {
        name.chars()
            .filter(char::is_ascii_alphanumeric)
            .map(|c| c.to_ascii_uppercase())
            .collect()
    };
    normalize(a) == normalize(b)
}

/// The native name a mapped field takes in a tag type, or `None` when the type cannot hold it.
fn mapped_native(mapping: &PicardMapping, tag_type: TagType) -> Option<&'static str> {
    match tag_type {
        TagType::Id3v2 => mapping.id3,
        TagType::Mp4Ilst => mapping.mp4,
        TagType::VorbisComments => Some(mapping.portable),
        _ => None,
    }
}

/// The portable name of a standard field: its Vorbis key where one exists, else its lofty name.
///
/// A Picard override wins over lofty's own key, so `EncoderSettings` is named `ENCODERSETTINGS`,
/// not lofty's `ENCODING`.
fn standard_name(key: ItemKey) -> String {
    if let Some(mapping) = picard_mapping_for_key(key) {
        return mapping.portable.to_string();
    }
    key.map_key(TagType::VorbisComments)
        .map(str::to_string)
        .unwrap_or_else(|| format!("{key:?}"))
}

/// Whether the target tag type holds a standard item. A key with no mapping, or a binary value in a
/// format that stores text only, cannot be carried. A Picard override decides the mapped fields,
/// so lofty's own mapping cannot hide a field that Picard names differently. The override is keyed
/// by [`ItemKey`], so two Picard names that share a Vorbis key, such as integer and fractional BPM,
/// are not conflated.
fn standard_is_supported(item: &TagItem, tag_type: TagType) -> bool {
    let text = matches!(item.value(), ItemValue::Text(_) | ItemValue::Locator(_));
    if !text {
        return false;
    }
    if let Some(mapping) = picard_mapping_for_key(item.key()) {
        return mapped_native(mapping, tag_type).is_some();
    }
    item.key().map_key(tag_type).is_some()
}

/// Whether the target format holds a custom field.
fn custom_is_supported(field: &CustomField, format: OutputFormat) -> bool {
    // A frame that held no value cannot cross formats; name it in the report.
    if field.values.is_empty() {
        return false;
    }
    match format {
        // A TXXX frame needs a non-empty description. A mapped field needs an ID3 name at all.
        OutputFormat::Wav => match picard_mapping(&field.name) {
            Some(mapping) => mapping.id3.is_some(),
            None => !field.name.is_empty(),
        },
        // A Vorbis key holds ASCII 0x20 through 0x7D, and no `=`. A Picard field whose Vorbis key is
        // parameterized or two-part has no plain key the copier can write, so it is named instead.
        OutputFormat::Flac | OutputFormat::Opus => {
            is_valid_vorbis_key(&field.name) && !is_parameterized_picard_field(&field.name)
        }
    }
}

/// Whether a custom field is a Picard field whose Vorbis key is parameterized or two-part, such as
/// `MusicIP Fingerprint` (`FINGERPRINT=MusicMagic Fingerprint {fingerprint}`). The copier names it
/// for a Vorbis target instead of writing it under a key that is not the documented one.
fn is_parameterized_picard_field(name: &str) -> bool {
    ["MusicMagic Fingerprint", "MusicIP Fingerprint"]
        .iter()
        .any(|candidate| same_field_name(candidate, name))
}

/// Whether a Vorbis comment accepts the key. This matches the library's own check, so the report
/// names every key that the writer would drop.
fn is_valid_vorbis_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .bytes()
            .all(|byte| (0x20..=0x7D).contains(&byte) && byte != b'=')
}

/// Read every tag and picture from a file.
pub fn read_tags(path: &Path) -> Result<TagSet, TagError> {
    read_tags_inner(path, None)
}

/// Read every tag and picture from a file whose name does not carry its format.
///
/// The write check reads the temporary output file back, and its name ends in `.tmp`, so the
/// extension cannot decide the format. `format` names the tag family instead. The reading path is
/// the same as [`read_tags`].
pub fn read_written_tags(path: &Path, format: OutputFormat) -> Result<TagSet, TagError> {
    read_tags_inner(path, Some(file_type_for(format)))
}

/// The file type an output format writes, for a reader that cannot use the file extension.
fn file_type_for(format: OutputFormat) -> FileType {
    match format {
        OutputFormat::Flac => FileType::Flac,
        OutputFormat::Wav => FileType::Wav,
        OutputFormat::Opus => FileType::Opus,
    }
}

/// Read every tag and picture from a file, with an optional file type when the extension is not
/// enough.
fn read_tags_inner(path: &Path, forced: Option<FileType>) -> Result<TagSet, TagError> {
    let tagged = match forced {
        Some(file_type) => lofty::probe::Probe::open(path)
            .and_then(|probe| probe.set_file_type(file_type).read())
            .map_err(|error| TagError::new(path, error.to_string()))?,
        None => {
            lofty::read_from_path(path).map_err(|error| TagError::new(path, error.to_string()))?
        }
    };
    let file_type = tagged.file_type();

    let mut set = TagSet::default();
    if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
        set.standard = tag.items().cloned().collect();
        set.pictures = tag.pictures().to_vec();
    }
    match file_type {
        // A WAV, AIFF, or AAC file carries ID3v2, so its custom fields are `TXXX` frames.
        FileType::Mpeg | FileType::Wav | FileType::Aiff | FileType::Aac => {
            let (custom, carried, lost) = read_id3_customs(path, file_type)?;
            set.custom = custom;
            set.carried = carried;
            set.lost = lost;
        }
        FileType::Mp4 => set.custom = read_mp4_customs(path)?,
        FileType::Flac | FileType::Opus | FileType::Vorbis | FileType::Speex => {
            set.custom = read_vorbis_customs(path, file_type)?;
        }
        _ => {}
    }
    correct_identities(path, file_type, &mut set)?;
    Ok(set)
}

/// Correct lofty's generic identity for the fields the Picard mapping distinguishes.
///
/// An ID3v2 tag stores the label in `TPUB`, and lofty reads that as `Publisher`. An ID3v2 tag has no
/// separate publisher frame, so every `Publisher` becomes `Label`. A Vorbis `ORIGINALYEAR` comment
/// shares lofty's `OriginalReleaseDate` with `ORIGINALDATE`, so it is split back out as its own
/// field.
fn correct_identities(path: &Path, file_type: FileType, set: &mut TagSet) -> Result<(), TagError> {
    // Picard's lyrics row is one field: the ID3 `USLT` frame and the Vorbis `LYRICS` key. lofty
    // splits it into `Lyrics` and `UnsyncLyrics`, so both become `UnsyncLyrics`, which the override
    // row maps back to `USLT` and `LYRICS`.
    //
    // Picard's BPM row is one field: the ID3 `TBPM` frame and the Vorbis `BPM` key. An integral BPM
    // becomes `IntegerBpm` so the override writes the ID3 frame. A fractional BPM stays `Bpm`, which
    // has no ID3 frame, so a target that cannot hold the exact text reports it instead of rounding.
    for item in &mut set.standard {
        if item.key() == ItemKey::Lyrics {
            *item = TagItem::new(ItemKey::UnsyncLyrics, item.value().clone());
        } else if item.key() == ItemKey::Bpm
            && item
                .value()
                .text()
                .is_some_and(|text| text.parse::<i64>().is_ok())
        {
            *item = TagItem::new(ItemKey::IntegerBpm, item.value().clone());
        }
    }
    match file_type {
        FileType::Mpeg | FileType::Wav | FileType::Aiff | FileType::Aac => {
            for item in &mut set.standard {
                if item.key() == ItemKey::Publisher {
                    *item = TagItem::new(ItemKey::Label, item.value().clone());
                }
            }
            // A carried frame stands in for a generic item that would otherwise mis-represent it:
            // a described `COMM` must not merge into `COMMENT`, a `POPM` must not become the
            // lossy `RATING` key, and a malformed `MVIN` must not become a guessed movement.
            let comment_bodies: Vec<String> = set
                .carried
                .iter()
                .filter_map(|carried| match &carried.frame {
                    Frame::Comment(frame) => Some(frame.content.to_string()),
                    _ => None,
                })
                .collect();
            let has_popm = set
                .carried
                .iter()
                .any(|carried| matches!(carried.frame, Frame::Popularimeter(_)));
            let malformed_movement = set.carried.iter().any(|carried| carried.name == "MVIN");
            set.standard.retain(|item| {
                let described_comment = item.key() == ItemKey::Comment
                    && item
                        .value()
                        .text()
                        .is_some_and(|text| comment_bodies.iter().any(|body| body == text));
                let rating = has_popm && item.key() == ItemKey::Popularimeter;
                let movement = malformed_movement
                    && (item.key() == ItemKey::MovementNumber
                        || item.key() == ItemKey::MovementTotal);
                !(described_comment || rating || movement)
            });
        }
        FileType::Flac | FileType::Opus | FileType::Vorbis | FileType::Speex => {
            let items = vorbis_items(path, file_type)?;
            let years: Vec<String> = native_values(&items, "ORIGINALYEAR");
            if !years.is_empty() {
                // Rebuild the original date from the native `ORIGINALDATE` comments only, so the
                // year value does not leak into it.
                let dates = native_values(&items, "ORIGINALDATE");
                set.standard
                    .retain(|item| item.key() != ItemKey::OriginalReleaseDate);
                for date in dates {
                    set.standard.push(TagItem::new(
                        ItemKey::OriginalReleaseDate,
                        ItemValue::Text(date),
                    ));
                }
                set.custom.push(CustomField {
                    name: "ORIGINALYEAR".to_string(),
                    values: years,
                });
            }
        }
        _ => {}
    }
    Ok(())
}

/// Every value of a native key, matched without case.
fn native_values(items: &[(String, String)], key: &str) -> Vec<String> {
    items
        .iter()
        .filter(|(candidate, _)| candidate.eq_ignore_ascii_case(key))
        .map(|(_, value)| value.clone())
        .collect()
}

/// The owner that marks a `UFID` frame as the MusicBrainz recording ID.
const MUSICBRAINZ_UFID_OWNER: &str = "http://musicbrainz.org";

/// What [`read_id3_customs`] returns: the custom fields, the frames carried verbatim, and the
/// fields the reader itself lost.
type Id3Customs = (Vec<CustomField>, Vec<CarriedFrame>, Vec<String>);

/// The custom fields of an ID3v2-carrying file: every `TXXX` frame whose description has no
/// standard mapping, plus every other frame the generic layer leaves behind. The frames the generic
/// model cannot represent are returned as carried frames, to be written verbatim into a WAV output
/// and named for a Vorbis target. The third value names a field the reader itself lost.
fn read_id3_customs(path: &Path, file_type: FileType) -> Result<Id3Customs, TagError> {
    let mut fields = Vec::new();
    let mut carried = Vec::new();
    let mut lost = Vec::new();
    let Some(id3) = read_id3_tag(path, file_type)? else {
        return Ok((fields, carried, lost));
    };
    // An ID3v2.4 tag may hold several `WOAR` frames, one per website. `lofty`'s reader keeps only
    // one, so a small walk over the raw tag counts them. When the raw tag holds more than the reader
    // exposes, the website field is named in the report instead of losing a URL in silence.
    if file_type == FileType::Mpeg {
        let exposed = id3.iter().filter(|frame| frame.id_str() == "WOAR").count();
        if raw_id3_frame_count(path, "WOAR").is_some_and(|raw| raw > exposed) {
            lost.push("WEBSITE".to_string());
        }
    }
    for frame in &id3 {
        match frame {
            Frame::UserText(ExtendedTextFrame {
                description,
                content,
                ..
            }) => {
                if let Some(mapping) = picard_mapping_for_id3_text(description) {
                    // A mapped `TXXX` frame, such as the script or the release status, crosses the
                    // formats under its portable name. When the generic layer also carries the frame
                    // as a standard item, that item keeps the value under the portable name, so a
                    // custom copy would duplicate every value.
                    if ItemKey::from_key(TagType::Id3v2, description).is_none() {
                        fields.push(CustomField {
                            name: mapping.portable.to_string(),
                            values: content.split('\0').map(str::to_string).collect(),
                        });
                    }
                } else if ItemKey::from_key(TagType::Id3v2, description).is_none() {
                    fields.push(CustomField {
                        name: description.to_string(),
                        values: content.split('\0').map(str::to_string).collect(),
                    });
                }
            }
            // A `UFID` with the MusicBrainz owner is the recording ID, which the generic layer
            // carries. Any other owner is a different field, so the frame is carried and named.
            Frame::UniqueFileIdentifier(ufid) => {
                if ufid.owner.as_ref() != MUSICBRAINZ_UFID_OWNER {
                    carried.push(CarriedFrame {
                        name: "UFID".to_string(),
                        frame: frame.clone(),
                    });
                }
            }
            // A `COMM` with a description is not the plain `COMMENT` field, so the frame is carried
            // and named rather than merged into the comment.
            Frame::Comment(comment) => {
                if !comment.description.is_empty() {
                    carried.push(CarriedFrame {
                        name: format!("COMM:{}", comment.description),
                        frame: frame.clone(),
                    });
                }
            }
            // The involved-people roles (`TIPL`/`IPLS`) reach the generic layer as standard items.
            // The musician-credits `TMCL` has no mapping at all, so the frame is carried.
            Frame::KeyValue(credits) => {
                if credits.id().as_str() == "TMCL" {
                    carried.push(CarriedFrame {
                        name: "TMCL".to_string(),
                        frame: frame.clone(),
                    });
                }
            }
            // Picard's rating row is the `RATING:user@email` Vorbis key, which the copier cannot
            // write without a user, so the frame is carried and named.
            Frame::Popularimeter(_) => {
                carried.push(CarriedFrame {
                    name: "POPM".to_string(),
                    frame: frame.clone(),
                });
            }
            // `MVIN` holds `number` or `number/count`. Anything else is malformed and is carried, so
            // it is reported instead of guessed.
            Frame::Text(text) if text.id().as_str() == "MVIN" => {
                if !is_movement_value(&text.value) {
                    carried.push(CarriedFrame {
                        name: "MVIN".to_string(),
                        frame: frame.clone(),
                    });
                }
            }
            Frame::Picture(_) => {}
            // Every remaining frame has no standard mapping, so nothing may drop it in silence.
            other => {
                if ItemKey::from_key(TagType::Id3v2, other.id_str()).is_none() {
                    fields.push(CustomField {
                        name: other.id_str().to_string(),
                        values: Vec::new(),
                    });
                }
            }
        }
    }
    Ok((fields, carried, lost))
}

/// The contents of every frame with an id in the raw ID3v2 tag at the start of a file, in order.
///
/// `lofty`'s reader can collapse several valid frames with the same id into one. A small walk over
/// the frame headers reaches them all, so the copier can count or read them. It adds no dependency
/// and covers the ID3v2.3 and 2.4 frame-size forms. It returns `None` when the file has no ID3v2
/// tag, so a caller can fall back to the reader's view.
fn raw_id3_frame_values(path: &Path, wanted: &str) -> Option<Vec<Vec<u8>>> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() < 10 || &bytes[0..3] != b"ID3" {
        return None;
    }
    let major = bytes[3];
    let syncsafe = |window: &[u8]| -> usize {
        window.iter().fold(0usize, |total, byte| {
            (total << 7) | (usize::from(*byte) & 0x7f)
        })
    };
    let end = 10 + syncsafe(&bytes[6..10]);
    let mut offset = 10;
    let mut values = Vec::new();
    while offset + 10 <= end && end <= bytes.len() {
        let id = &bytes[offset..offset + 4];
        if id == [0, 0, 0, 0] {
            break;
        }
        let size = if major >= 4 {
            syncsafe(&bytes[offset + 4..offset + 8])
        } else {
            (usize::from(bytes[offset + 4]) << 24)
                | (usize::from(bytes[offset + 5]) << 16)
                | (usize::from(bytes[offset + 6]) << 8)
                | usize::from(bytes[offset + 7])
        };
        let start = offset + 10;
        let stop = start + size;
        if stop > end || stop > bytes.len() {
            break;
        }
        if id == wanted.as_bytes() {
            values.push(bytes[start..stop].to_vec());
        }
        offset = stop;
    }
    Some(values)
}

/// The number of frames with an id in the raw ID3v2 tag at the start of a file.
fn raw_id3_frame_count(path: &Path, wanted: &str) -> Option<usize> {
    raw_id3_frame_values(path, wanted).map(|values| values.len())
}

/// Whether an ID3 `MVIN` value is `number` or `number/count`, the two forms Picard defines.
fn is_movement_value(value: &str) -> bool {
    let mut parts = value.splitn(2, '/');
    if parts.next().unwrap_or_default().parse::<u32>().is_err() {
        return false;
    }
    match parts.next() {
        None => true,
        Some(count) => count.parse::<u32>().is_ok(),
    }
}

/// The ID3v2 tag of an MP3, WAV, AIFF, or AAC file.
fn read_id3_tag(
    path: &Path,
    file_type: FileType,
) -> Result<Option<lofty::id3::v2::Id3v2Tag>, TagError> {
    let failed = |error: lofty::error::FileParseError| TagError::new(path, error.to_string());
    let mut file = File::open(path).map_err(|error| TagError::new(path, error.to_string()))?;
    let options = ParseOptions::new();
    let tag = match file_type {
        FileType::Mpeg => lofty::mpeg::MpegFile::read_from(&mut file, options)
            .map_err(failed)?
            .id3v2()
            .cloned(),
        FileType::Wav => lofty::iff::wav::WavFile::read_from(&mut file, options)
            .map_err(failed)?
            .id3v2()
            .cloned(),
        FileType::Aiff => lofty::iff::aiff::AiffFile::read_from(&mut file, options)
            .map_err(failed)?
            .id3v2()
            .cloned(),
        FileType::Aac => lofty::aac::AacFile::read_from(&mut file, options)
            .map_err(failed)?
            .id3v2()
            .cloned(),
        _ => None,
    };
    Ok(tag)
}

/// The custom fields of an M4A: every freeform atom whose name has no standard mapping, plus every
/// other atom the generic layer leaves behind.
fn read_mp4_customs(path: &Path) -> Result<Vec<CustomField>, TagError> {
    let mut file = File::open(path).map_err(|error| TagError::new(path, error.to_string()))?;
    let mp4 = lofty::mp4::Mp4File::read_from(&mut file, ParseOptions::new())
        .map_err(|error| TagError::new(path, error.to_string()))?;

    let mut fields = Vec::new();
    if let Some(ilst) = mp4.ilst() {
        for atom in ilst {
            let ident = atom.ident();
            // The library matches an atom by this string: a freeform atom keeps the
            // `----:mean:name` of its identifier, a four-byte atom its four characters.
            let (key, name) = match ident {
                AtomIdent::Freeform { mean, name } => {
                    let portable = picard_mapping_for_mp4(name)
                        .map(|mapping| mapping.portable.to_string())
                        .unwrap_or_else(|| name.to_string());
                    (format!("----:{mean}:{name}"), portable)
                }
                AtomIdent::Fourcc(fourcc) => (
                    fourcc.iter().map(|byte| char::from(*byte)).collect(),
                    fourcc.iter().map(|byte| char::from(*byte)).collect(),
                ),
            };
            if ItemKey::from_key(TagType::Mp4Ilst, &key).is_some() {
                continue;
            }
            values_from_atom(atom, &mut fields, name);
        }
    }
    Ok(fields)
}

/// Add an atom's text values as a custom field, or record its name alone when it holds no text.
fn values_from_atom(atom: &lofty::mp4::Atom<'static>, fields: &mut Vec<CustomField>, name: String) {
    let mut values = Vec::new();
    for data in atom.data() {
        match data {
            AtomData::UTF8(text) | AtomData::UTF16(text) => values.push(text.clone()),
            _ => {}
        }
    }
    fields.push(CustomField { name, values });
}

/// Every key and value of a Vorbis comment tag.
fn vorbis_items(path: &Path, file_type: FileType) -> Result<Vec<(String, String)>, TagError> {
    let mut file = File::open(path).map_err(|error| TagError::new(path, error.to_string()))?;
    let options = ParseOptions::new();
    let items: Vec<(String, String)> = match file_type {
        FileType::Flac => {
            let flac = lofty::flac::FlacFile::read_from(&mut file, options)
                .map_err(|error| TagError::new(path, error.to_string()))?;
            flac.vorbis_comments().map(items_of).unwrap_or_default()
        }
        FileType::Opus => {
            let opus = lofty::ogg::OpusFile::read_from(&mut file, options)
                .map_err(|error| TagError::new(path, error.to_string()))?;
            items_of(opus.vorbis_comments())
        }
        FileType::Vorbis => {
            let vorbis = lofty::ogg::VorbisFile::read_from(&mut file, options)
                .map_err(|error| TagError::new(path, error.to_string()))?;
            items_of(vorbis.vorbis_comments())
        }
        FileType::Speex => {
            let speex = lofty::ogg::SpeexFile::read_from(&mut file, options)
                .map_err(|error| TagError::new(path, error.to_string()))?;
            items_of(speex.vorbis_comments())
        }
        _ => Vec::new(),
    };
    Ok(items)
}

/// The custom fields of a FLAC, Opus, Vorbis, or Speex file: every comment key with no standard
/// mapping.
fn read_vorbis_customs(path: &Path, file_type: FileType) -> Result<Vec<CustomField>, TagError> {
    let items = vorbis_items(path, file_type)?;

    let mut fields: Vec<CustomField> = Vec::new();
    for (key, value) in items {
        // `RATING:email` is the generic layer's popularimeter field, not a custom field.
        if key.starts_with("RATING:") {
            continue;
        }
        if ItemKey::from_key(TagType::VorbisComments, &key).is_some() {
            continue;
        }
        match fields.iter_mut().find(|field| field.name == key) {
            Some(field) => field.values.push(value),
            None => fields.push(CustomField {
                name: key,
                values: vec![value],
            }),
        }
    }
    Ok(fields)
}

/// Every key and value of a Vorbis comments tag.
fn items_of(comments: &VorbisComments) -> Vec<(String, String)> {
    comments
        .items()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

/// Write a tag set into an existing file in the output's own tag format.
///
/// The caller reads [`TagSet::unsupported_for`] for the fields the target cannot hold.
pub fn write_into(path: &Path, format: OutputFormat, tags: &TagSet) -> Result<(), TagError> {
    match format {
        OutputFormat::Wav => write_id3(path, tags),
        OutputFormat::Flac | OutputFormat::Opus => write_vorbis(path, tags),
    }
}

/// The source fields whose values the written file does not hold.
///
/// The caller reads the output back with [`read_written_tags`] and compares it with the source. The
/// comparison is in the portable key space the copier uses, counting multiplicity, so a loss inside
/// a writer is named in the report instead of disappearing. A field that a writer is known to
/// transform (the movement pair into one `MVIN`, a license URL into `WCOP`, a date) is compared in
/// its canonical form, so a deliberate transform counts as preserved.
pub fn write_losses(source: &TagSet, output: &TagSet) -> Vec<String> {
    let expected = canonical_values(source);
    let produced = canonical_values(output);
    let mut lost = Vec::new();
    for (name, values) in &expected {
        let holds = produced
            .get(name)
            .is_some_and(|written| multiset_holds(written, values));
        if !holds && !lost.contains(name) {
            lost.push(name.clone());
        }
    }
    lost
}

/// The `(name, value)` pairs of a tag set in the portable key space: a standard field by its Picard
/// or Vorbis key, a custom field by its own name. One pair per value; a carried frame is not a
/// portable field, so it is left out.
fn portable_values(tags: &TagSet) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for item in &tags.standard {
        let value = item
            .value()
            .text()
            .or_else(|| item.value().locator())
            .unwrap_or_default()
            .to_string();
        pairs.push((standard_name(item.key()), value));
    }
    for field in &tags.custom {
        for value in &field.values {
            pairs.push((field.name.clone(), value.clone()));
        }
    }
    pairs
}

/// The portable values of a tag set, grouped by field name and sorted, with the movement pair
/// joined into the one `MOVEMENT` field the copier uses. A value is split on the ID3v2.4
/// multi-value separator, so one null-separated frame counts as its separate values.
fn canonical_values(tags: &TagSet) -> BTreeMap<String, Vec<String>> {
    let mut numbers = Vec::new();
    let mut totals = Vec::new();
    let mut fields: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (name, value) in portable_values(tags) {
        for part in value.split('\0') {
            match name.as_str() {
                "MOVEMENT" => numbers.push(part.to_string()),
                "MOVEMENTTOTAL" => totals.push(part.to_string()),
                _ => fields
                    .entry(name.clone())
                    .or_default()
                    .push(part.to_string()),
            }
        }
    }
    for token in movement_tokens(&numbers, &totals) {
        fields
            .entry("MOVEMENT".to_string())
            .or_default()
            .push(token);
    }
    for values in fields.values_mut() {
        values.sort();
    }
    fields
}

/// The `MVIN` tokens of a movement number and total list. A number with a total at the same
/// position joins as `number/total`, a number already carrying a slash stays, a bare number stays,
/// and a total with no number becomes `?/total` because Picard's `MVIN` needs a number.
fn movement_tokens(numbers: &[String], totals: &[String]) -> Vec<String> {
    let mut tokens = Vec::new();
    for (index, number) in numbers.iter().enumerate() {
        if number.contains('/') {
            tokens.push(number.clone());
        } else if let Some(total) = totals.get(index) {
            tokens.push(format!("{number}/{total}"));
        } else {
            tokens.push(number.clone());
        }
    }
    for total in totals.iter().skip(numbers.len()) {
        tokens.push(format!("?/{total}"));
    }
    tokens
}

/// Whether `written` holds every value of `expected`, counting multiplicity. An extra value in
/// `written` does not matter, because the check reports a loss, not an addition.
fn multiset_holds(written: &[String], expected: &[String]) -> bool {
    let mut available = written.to_vec();
    for value in expected {
        let Some(position) = available.iter().position(|candidate| candidate == value) else {
            return false;
        };
        available.remove(position);
    }
    true
}

/// Add a value to a `TXXX` frame's group, keyed by its exact description. The standard and the
/// custom representation of one description carry the same native name, so they land in the same
/// group; two distinct descriptions stay apart, so no identity merges.
fn push_user_text(groups: &mut Vec<(String, Vec<String>)>, description: &str, value: String) {
    match groups.iter_mut().find(|(name, _)| name == description) {
        Some((_, values)) => values.push(value),
        None => groups.push((description.to_string(), vec![value])),
    }
}

/// Write ID3v2 to a WAV output.
///
/// A multi-value field is one frame whose values are null-separated, as ID3v2.4 defines a
/// multi-value text frame. That is how `ARTISTS`, `WRITER`, `WORK`, the record label, and the
/// license keep every value; one `WOAR` frame is written per website, because a URL frame holds a
/// single URL.
fn write_id3(path: &Path, tags: &TagSet) -> Result<(), TagError> {
    let mut tag = Tag::new(TagType::Id3v2);
    let mut user_text: Vec<(String, Vec<String>)> = Vec::new();
    let mut movement_number: Option<String> = None;
    let mut movement_total: Option<String> = None;
    let mut license: Vec<String> = Vec::new();
    let mut labels: Vec<String> = Vec::new();
    let mut websites: Vec<String> = Vec::new();
    let mut other: Vec<(String, String)> = Vec::new();
    for item in &tags.standard {
        // Picard's movement row is one `MVIN` frame, `number` or `number/count`. The two generic
        // items are joined into one frame below.
        match item.key() {
            ItemKey::MovementNumber => {
                movement_number = item.value().text().map(str::to_string);
                continue;
            }
            ItemKey::MovementTotal => {
                movement_total = item.value().text().map(str::to_string);
                continue;
            }
            // Picard's license row arrives two ways: a Vorbis `LICENSE` comment reads as `License`,
            // and an ID3 `WCOP` URL frame reads as `CopyrightUrl`. Every value is collected here,
            // from both forms, and the whole row is written once below.
            ItemKey::License | ItemKey::CopyrightUrl => {
                if let Some(value) = item.value().text().or_else(|| item.value().locator()) {
                    license.push(value.to_string());
                }
                continue;
            }
            // Picard's record label is the ID3 `TPUB` frame. The generic conversion keeps one value,
            // so every label is collected and written as one null-separated frame below.
            ItemKey::Label => {
                if let Some(value) = item.value().text() {
                    labels.push(value.to_string());
                }
                continue;
            }
            // Picard's artist website is the ID3 `WOAR` URL frame. A URL frame holds one URL, so
            // every website is collected and written as its own frame below.
            ItemKey::TrackArtistUrl => {
                if let Some(value) = item.value().text().or_else(|| item.value().locator()) {
                    websites.push(value.to_string());
                }
                continue;
            }
            _ => {}
        }
        if let Some(mapping) = picard_mapping_for_key(item.key()) {
            // A mapped field with a `TXXX` native name is written by hand, because lofty's generic
            // layer has no ID3 frame for it. Every value is collected, so they are written together.
            if mapping.id3_user_text {
                if let (Some(value), Some(native)) = (item.value().text(), mapping.id3) {
                    push_user_text(&mut user_text, native, value.to_string());
                }
                continue;
            }
        }
        if standard_is_supported(item, TagType::Id3v2) {
            tag.push(item.clone());
        }
    }
    for field in &tags.custom {
        if !custom_is_supported(field, OutputFormat::Wav) {
            continue;
        }
        // Picard's license and website rows are written from all of their values together, so the
        // custom representation reaches the same frames as the standard one.
        let portable = picard_mapping(&field.name).map(|mapping| mapping.portable);
        if portable == Some("LICENSE") {
            license.extend(field.values.iter().cloned());
            continue;
        }
        if portable == Some("WEBSITE") {
            websites.extend(field.values.iter().cloned());
            continue;
        }
        // A mapped custom field uses its native ID3 name: a user-text frame is identified by its
        // description, and any other frame by its id.
        match picard_mapping(&field.name) {
            Some(mapping) if mapping.id3_user_text => {
                if let Some(native) = mapping.id3 {
                    for value in &field.values {
                        push_user_text(&mut user_text, native, value.clone());
                    }
                }
            }
            Some(mapping) => {
                if let Some(native) = mapping.id3 {
                    other.push((native.to_string(), field.values.join("\0")));
                }
            }
            None => {
                for value in &field.values {
                    push_user_text(&mut user_text, &field.name, value.clone());
                }
            }
        }
    }
    for picture in &tags.pictures {
        tag.push_picture(picture.clone());
    }

    let mut id3 = lofty::id3::v2::Id3v2Tag::from(tag);
    // One `TXXX` frame per description, its values null-separated. The standard and custom
    // representations of a description are already combined, so no value replaces another.
    for (description, values) in user_text {
        id3.insert_user_text(description, values.join("\0"));
    }
    // One `TPUB` frame for the record label, its values null-separated.
    if !labels.is_empty() {
        id3.insert(Frame::Text(TextInformationFrame::new(
            FrameId::Valid("TPUB".to_string().into()),
            TextEncoding::UTF8,
            labels.join("\0"),
        )));
    }
    // Picard's website row becomes one `WOAR` frame. `Id3v2Tag` cannot hold two frames with the
    // same id: `UrlLinkFrame`'s equality and hash use only `header.id`
    // (`lofty-0.25.4/src/id3/v2/items/url_link_frame.rs:18-28`), and the only way to add a frame,
    // `FrameList::push`, is crate private (`lofty-0.25.4/src/id3/v2/frame/list.rs:264`). So the first
    // website is written, and the write check names `WEBSITE` in the returned list for the rest.
    if let Some(url) = websites.into_iter().next() {
        id3.insert(Frame::Url(UrlLinkFrame::new(
            FrameId::Valid("WOAR".to_string().into()),
            url,
        )));
    }
    for (native, value) in other {
        insert_id3_text_or_url(&mut id3, &native, value);
    }
    if let Some(number) = movement_number {
        let value = match movement_total {
            Some(total) => format!("{number}/{total}"),
            None => number,
        };
        id3.insert(Frame::Text(TextInformationFrame::new(
            FrameId::Valid("MVIN".to_string().into()),
            TextEncoding::UTF8,
            value,
        )));
    }
    // Picard's license row: one `WCOP` URL frame for a single URL, and one `TXXX:LICENSE` frame
    // otherwise. Two or more values become one null-separated `TXXX:LICENSE` frame, as ID3v2.4
    // defines a multi-value text frame, so no value is dropped.
    if !license.is_empty() {
        if license.len() == 1 && is_http_url(&license[0]) {
            id3.insert(Frame::Url(UrlLinkFrame::new(
                FrameId::Valid("WCOP".to_string().into()),
                license[0].clone(),
            )));
        } else {
            id3.insert_user_text("LICENSE".to_string(), license.join("\0"));
        }
    }
    // Re-insert the frames the generic model cannot represent, so a WAV output keeps them.
    for carried in &tags.carried {
        id3.insert(carried.frame.clone());
    }

    id3.save_to_path(path, WriteOptions::default())
        .map_err(|error| TagError::new(path, format!("write the ID3 tag: {error}")))
}

/// Whether a license value is a URL, which ID3 stores in the `WCOP` frame rather than a `TXXX`.
fn is_http_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

/// Insert a text value under an ID3v2 frame id: a URL frame for a `W...` id, a text frame
/// otherwise.
fn insert_id3_text_or_url(id3: &mut lofty::id3::v2::Id3v2Tag, id: &str, value: String) {
    let frame_id = FrameId::Valid(id.to_string().into());
    let frame = if id.len() == 4 && id.starts_with('W') {
        Frame::Url(UrlLinkFrame::new(frame_id, value))
    } else {
        Frame::Text(TextInformationFrame::new(
            frame_id,
            TextEncoding::UTF8,
            value,
        ))
    };
    id3.insert(frame);
}

/// Write Vorbis comments to a FLAC or Opus output.
///
/// The `EncoderSoftware` field is written as the `ENCODER` comment rather than as the tag's vendor.
/// The Ogg writer keeps the file's own vendor string and ignores the one on the tag, and melomae
/// writes its own vendor, so the field must be a comment to survive.
fn write_vorbis(path: &Path, tags: &TagSet) -> Result<(), TagError> {
    let mut tag = Tag::new(TagType::VorbisComments);
    let mut mapped: Vec<(String, String)> = Vec::new();
    for item in &tags.standard {
        if item.key() == ItemKey::EncoderSoftware {
            continue;
        }
        if !standard_is_supported(item, TagType::VorbisComments) {
            continue;
        }
        // A Picard override decides the literal Vorbis key. Lofty's own conversion would name the
        // field differently: it writes `ENCODING` for the encoder settings where Picard uses
        // `ENCODERSETTINGS`, and `INITIALKEY` for the initial key where Picard uses `KEY`. A mapped
        // value is written by hand so the override wins; a URL field carries a locator, not text.
        if let Some(mapping) = picard_mapping_for_key(item.key()) {
            if let Some(value) = item.value().text().or_else(|| item.value().locator()) {
                mapped.push((mapping.portable.to_string(), value.to_string()));
                continue;
            }
        }
        tag.push(item.clone());
    }
    for picture in &tags.pictures {
        tag.push_picture(picture.clone());
    }

    let mut comments = VorbisComments::from(tag);
    comments.set_vendor(concat!("melomae ", env!("CARGO_PKG_VERSION")).to_string());
    for (name, value) in mapped {
        comments.push(name, value);
    }
    for item in &tags.standard {
        if item.key() != ItemKey::EncoderSoftware {
            continue;
        }
        if let Some(value) = item.value().text() {
            comments.push("ENCODER".to_string(), value.to_string());
        }
    }
    for field in &tags.custom {
        if !custom_is_supported(field, OutputFormat::Flac) {
            continue;
        }
        for value in &field.values {
            comments.push(field.name.clone(), value.clone());
        }
    }

    comments
        .save_to_path(path, WriteOptions::default())
        .map_err(|error| TagError::new(path, format!("write the Vorbis comments: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(key: ItemKey, value: &str) -> TagItem {
        TagItem::new(key, ItemValue::Text(value.to_string()))
    }

    fn custom(name: &str, values: &[&str]) -> CustomField {
        CustomField {
            name: name.to_string(),
            values: values.iter().map(|value| (*value).to_string()).collect(),
        }
    }

    fn set(standard: Vec<TagItem>, custom: Vec<CustomField>) -> TagSet {
        TagSet {
            standard,
            custom,
            pictures: Vec::new(),
            carried: Vec::new(),
            lost: Vec::new(),
        }
    }

    #[test]
    fn a_vorbis_key_holds_ascii_and_no_equals_sign() {
        assert!(is_valid_vorbis_key("MYCUSTOM"));
        assert!(is_valid_vorbis_key("A B"));
        assert!(!is_valid_vorbis_key(""));
        assert!(!is_valid_vorbis_key("BAD=KEY"));
        assert!(!is_valid_vorbis_key("caf\u{e9}"));
    }

    #[test]
    fn a_custom_field_that_vorbis_cannot_hold_is_reported() {
        let tags = set(Vec::new(), vec![custom("BAD=KEY", &["v"])]);
        assert_eq!(
            tags.unsupported_for(OutputFormat::Flac),
            vec!["BAD=KEY".to_string()]
        );
        assert_eq!(
            tags.unsupported_for(OutputFormat::Opus),
            vec!["BAD=KEY".to_string()]
        );
        assert!(tags.unsupported_for(OutputFormat::Wav).is_empty());
    }

    #[test]
    fn a_custom_field_with_no_value_cannot_cross_formats() {
        let tags = set(Vec::new(), vec![custom("EMPTY", &[])]);
        assert_eq!(
            tags.unsupported_for(OutputFormat::Wav),
            vec!["EMPTY".to_string()]
        );
        assert_eq!(
            tags.unsupported_for(OutputFormat::Opus),
            vec!["EMPTY".to_string()]
        );
    }

    #[test]
    fn a_standard_field_with_no_target_mapping_is_reported() {
        // EBU R128 gain exists as a Vorbis key, but ID3v2 has no frame for it.
        let tags = set(vec![item(ItemKey::R128TrackGain, "-3.0 dB")], Vec::new());
        assert_eq!(
            tags.unsupported_for(OutputFormat::Wav),
            vec!["R128_TRACK_GAIN".to_string()]
        );
        assert!(tags.unsupported_for(OutputFormat::Flac).is_empty());
    }

    #[test]
    fn the_report_names_a_standard_field_by_its_portable_key() {
        assert_eq!(standard_name(ItemKey::AlbumArtist), "ALBUMARTIST");
        assert_eq!(
            standard_name(ItemKey::MusicBrainzRecordingId),
            "MUSICBRAINZ_TRACKID"
        );
    }

    #[test]
    fn the_encoder_settings_use_the_picard_vorbis_key() {
        // Picard names the Vorbis key `ENCODERSETTINGS`; lofty's generic key for the same field is
        // `ENCODING`, so the override must win. The ID3 frame stays `TSSE`.
        assert_eq!(standard_name(ItemKey::EncoderSettings), "ENCODERSETTINGS");
        let tags = set(
            vec![item(ItemKey::EncoderSettings, "Fixture Encoder Settings")],
            Vec::new(),
        );
        assert!(standard_is_supported(
            &tags.standard[0],
            TagType::VorbisComments
        ));
        assert!(standard_is_supported(&tags.standard[0], TagType::Id3v2));
        assert!(tags.unsupported_for(OutputFormat::Flac).is_empty());
        assert!(tags.unsupported_for(OutputFormat::Wav).is_empty());
    }

    #[test]
    fn the_mapping_carries_a_field_between_the_three_output_formats() {
        // The album artist is TPE2 in ID3, aART in MP4, and ALBUMARTIST in Vorbis. The portable
        // name is the Vorbis key, and the target mapping is what the writers use.
        let tags = set(vec![item(ItemKey::AlbumArtist, "An Artist")], Vec::new());
        assert_eq!(standard_name(ItemKey::AlbumArtist), "ALBUMARTIST");
        assert!(standard_is_supported(
            &tags.standard[0],
            TagType::VorbisComments
        ));
        assert!(standard_is_supported(&tags.standard[0], TagType::Id3v2));
        assert!(tags.unsupported_for(OutputFormat::Wav).is_empty());
        assert!(tags.unsupported_for(OutputFormat::Flac).is_empty());
    }
}
