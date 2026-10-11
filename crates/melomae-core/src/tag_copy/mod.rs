//! Copy every tag and picture from a source file to an output file.
//!
//! [`read_tags`] reads a file's tags with `lofty`, carries the standard fields in lofty's generic
//! [`Tag`] model, and keeps the fields that the generic layer cannot hold (ID3 `TXXX`, Vorbis keys
//! with no [`ItemKey`], MP4 freeform atoms) as named custom fields. [`write_into`] writes a
//! [`TagSet`] into an output file in the output's own tag format: ID3 for WAV, Vorbis comments for
//! FLAC and Opus.
//!
//! The standard-field mapping is lofty's, which follows the MusicBrainz Picard tag mapping. A
//! field that the target format cannot hold is never dropped in silence: [`TagSet::unsupported_for`]
//! names it.

use std::fmt;
use std::fs::File;
use std::path::{Path, PathBuf};

use lofty::config::{ParseOptions, WriteOptions};
use lofty::file::{AudioFile, FileType, TaggedFileExt};
use lofty::id3::v2::{ExtendedTextFrame, Frame};
use lofty::mp4::{AtomData, AtomIdent};
use lofty::ogg::tag::VorbisComments;
use lofty::picture::Picture;
use lofty::tag::{ItemKey, ItemValue, Tag, TagExt, TagItem, TagType};

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

/// Every tag and picture read from one file.
#[derive(Debug, Clone, Default)]
pub struct TagSet {
    standard: Vec<TagItem>,
    custom: Vec<CustomField>,
    pictures: Vec<Picture>,
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
    /// dropped in silence.
    pub fn unsupported_for(&self, format: OutputFormat) -> Vec<String> {
        let tag_type = target_tag_type(format);
        let mut names = Vec::new();
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
/// Vorbis key. `id3` is the ID3v2 name, either a frame id or, when `id3_user_text` is set, a
/// `TXXX` description. `mp4` is the MP4 freeform atom name, without the
/// `----:com.apple.iTunes:` prefix.
struct PicardMapping {
    portable: &'static str,
    id3: Option<&'static str>,
    id3_user_text: bool,
    mp4: Option<&'static str>,
}

/// The fields the MusicBrainz Picard tag mapping names differently from lofty.
///
/// `LABEL`: Picard maps ID3 `TPUB` to Vorbis `LABEL`, but lofty reads `TPUB` as `Publisher` and
/// writes Vorbis `PUBLISHER`. `SCRIPT` and `RELEASESTATUS` have no ID3 `ItemKey` in lofty, so the
/// generic layer reports or mis-names them. `ORIGINALYEAR` shares lofty's `OriginalReleaseDate`
/// with `ORIGINALDATE`, and Picard defines no ID3 frame for it.
///
/// Source: <https://picard-docs.musicbrainz.org/en/latest/appendices/tag_mapping.html>, checked
/// 2026-10-11.
const PICARD_MAPPINGS: &[PicardMapping] = &[
    PicardMapping {
        portable: "LABEL",
        id3: Some("TPUB"),
        id3_user_text: false,
        mp4: Some("LABEL"),
    },
    PicardMapping {
        portable: "SCRIPT",
        id3: Some("SCRIPT"),
        id3_user_text: true,
        mp4: Some("SCRIPT"),
    },
    PicardMapping {
        portable: "RELEASESTATUS",
        id3: Some("MusicBrainzAlbumStatus"),
        id3_user_text: true,
        mp4: Some("MusicBrainzAlbumStatus"),
    },
    PicardMapping {
        portable: "ORIGINALYEAR",
        id3: None,
        id3_user_text: false,
        mp4: None,
    },
];

/// The mapping for a portable (Vorbis) name.
fn picard_mapping(portable: &str) -> Option<&'static PicardMapping> {
    PICARD_MAPPINGS.iter().find(|m| m.portable == portable)
}

/// The mapping for an ID3 `TXXX` description. The comparison ignores case, spaces, and
/// underscores, so both the compact Picard name and the spaced lofty name match.
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
fn standard_name(key: ItemKey) -> String {
    key.map_key(TagType::VorbisComments)
        .map(str::to_string)
        .unwrap_or_else(|| format!("{key:?}"))
}

/// Whether the target tag type holds a standard item. A key with no mapping, or a binary value in a
/// format that stores text only, cannot be carried. A Picard override decides the mapped fields,
/// so lofty's own mapping cannot hide a field that Picard names differently.
fn standard_is_supported(item: &TagItem, tag_type: TagType) -> bool {
    let text = matches!(item.value(), ItemValue::Text(_) | ItemValue::Locator(_));
    if !text {
        return false;
    }
    if let Some(mapping) = picard_mapping(&standard_name(item.key())) {
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
        // A Vorbis key holds ASCII 0x20 through 0x7D, and no `=`.
        OutputFormat::Flac | OutputFormat::Opus => is_valid_vorbis_key(&field.name),
    }
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
    let tagged =
        lofty::read_from_path(path).map_err(|error| TagError::new(path, error.to_string()))?;
    let file_type = tagged.file_type();

    let mut set = TagSet::default();
    if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
        set.standard = tag.items().cloned().collect();
        set.pictures = tag.pictures().to_vec();
    }
    set.custom = match file_type {
        // A WAV, AIFF, or AAC file carries ID3v2, so its custom fields are `TXXX` frames.
        FileType::Mpeg | FileType::Wav | FileType::Aiff | FileType::Aac => {
            read_id3_customs(path, file_type)?
        }
        FileType::Mp4 => read_mp4_customs(path)?,
        FileType::Flac | FileType::Opus | FileType::Vorbis | FileType::Speex => {
            read_vorbis_customs(path, file_type)?
        }
        _ => Vec::new(),
    };
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
    match file_type {
        FileType::Mpeg | FileType::Wav | FileType::Aiff | FileType::Aac => {
            for item in &mut set.standard {
                if item.key() == ItemKey::Publisher {
                    *item = TagItem::new(ItemKey::Label, item.value().clone());
                }
            }
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

/// The custom fields of an ID3v2-carrying file: every `TXXX` frame whose description has no
/// standard mapping, plus every other frame the generic layer leaves behind.
fn read_id3_customs(path: &Path, file_type: FileType) -> Result<Vec<CustomField>, TagError> {
    let mut fields = Vec::new();
    let Some(id3) = read_id3_tag(path, file_type)? else {
        return Ok(fields);
    };
    for frame in &id3 {
        match frame {
            Frame::UserText(ExtendedTextFrame {
                description,
                content,
                ..
            }) => {
                if let Some(mapping) = picard_mapping_for_id3_text(description) {
                    // A mapped `TXXX` frame, such as the script or the release status, crosses the
                    // formats under its portable name.
                    fields.push(CustomField {
                        name: mapping.portable.to_string(),
                        values: content.split('\0').map(str::to_string).collect(),
                    });
                } else if ItemKey::from_key(TagType::Id3v2, description).is_none() {
                    fields.push(CustomField {
                        name: description.to_string(),
                        values: content.split('\0').map(str::to_string).collect(),
                    });
                }
            }
            // Every remaining frame has no standard mapping, so nothing may drop it in silence.
            Frame::Picture(_) => {}
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
    Ok(fields)
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

/// Write ID3v2 to a WAV output.
fn write_id3(path: &Path, tags: &TagSet) -> Result<(), TagError> {
    let mut tag = Tag::new(TagType::Id3v2);
    let mut user_text: Vec<(String, String)> = Vec::new();
    for item in &tags.standard {
        if let Some(mapping) = picard_mapping(&standard_name(item.key())) {
            // A mapped field with a `TXXX` native name is written by hand, because lofty's generic
            // layer has no ID3 frame for it.
            if mapping.id3_user_text {
                if let (Some(value), Some(native)) = (item.value().text(), mapping.id3) {
                    user_text.push((native.to_string(), value.to_string()));
                }
                continue;
            }
        }
        if standard_is_supported(item, TagType::Id3v2) {
            tag.push(item.clone());
        }
    }
    for picture in &tags.pictures {
        tag.push_picture(picture.clone());
    }

    let mut id3 = lofty::id3::v2::Id3v2Tag::from(tag);
    for (name, value) in user_text {
        id3.insert_user_text(name, value);
    }
    for field in &tags.custom {
        if !custom_is_supported(field, OutputFormat::Wav) {
            continue;
        }
        // A mapped custom field, such as the release status, uses its native ID3 name. A TXXX
        // frame is identified by its description, so a multi-value field is one frame whose values
        // are null-separated, exactly as ID3v2.4 defines it.
        let name = picard_mapping(&field.name)
            .and_then(|mapping| mapping.id3)
            .unwrap_or(&field.name)
            .to_string();
        id3.insert_user_text(name, field.values.join("\0"));
    }

    id3.save_to_path(path, WriteOptions::default())
        .map_err(|error| TagError::new(path, format!("write the ID3 tag: {error}")))
}

/// Write Vorbis comments to a FLAC or Opus output.
///
/// The `EncoderSoftware` field is written as the `ENCODER` comment rather than as the tag's vendor.
/// The Ogg writer keeps the file's own vendor string and ignores the one on the tag, and melomae
/// writes its own vendor, so the field must be a comment to survive.
fn write_vorbis(path: &Path, tags: &TagSet) -> Result<(), TagError> {
    let mut tag = Tag::new(TagType::VorbisComments);
    for item in &tags.standard {
        if item.key() == ItemKey::EncoderSoftware {
            continue;
        }
        if standard_is_supported(item, TagType::VorbisComments) {
            tag.push(item.clone());
        }
    }
    for picture in &tags.pictures {
        tag.push_picture(picture.clone());
    }

    let mut comments = VorbisComments::from(tag);
    comments.set_vendor(concat!("melomae ", env!("CARGO_PKG_VERSION")).to_string());
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
