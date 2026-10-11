//! Build the task 15 tag fixtures: one MP3, one M4A, and one FLAC.
//!
//! The audio and the JPEG cover come from the pinned ffmpeg. ffmpeg also writes eight standard
//! fields with its own key names. The remaining fields, which ffmpeg writes with keys that the tag
//! reader does not map, come from lofty's format-specific API. The encoder metadata that ffmpeg
//! adds is removed, so each fixture holds exactly the fields the task lists.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use lofty::config::{ParseOptions, WriteOptions};
use lofty::file::AudioFile;
use lofty::id3::v2::{Frame, Id3v2Tag};
use lofty::mp4::{AtomData, AtomIdent, Ilst};
use lofty::ogg::OggPictureStorage;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::TagExt;
use lofty::tag::{ItemKey, ItemValue, Tag, TagItem, TagType};

use super::{ffmpeg, TempDir};

/// The fixture's field values. Every value is made up.
pub const TITLE: &str = "Fixture Title";
/// The artist.
pub const ARTIST: &str = "Fixture Artist";
/// The album.
pub const ALBUM: &str = "Fixture Album";
/// The album artist.
pub const ALBUM_ARTIST: &str = "Fixture Album Artist";
/// The release date.
pub const DATE: &str = "2020-01-02";
/// The genre.
pub const GENRE: &str = "Fixture Genre";
/// The track number and total, as one ID3v2 pair.
pub const TRACK: &str = "3";
/// The track total.
pub const TRACK_TOTAL: &str = "12";
/// The disc number.
pub const DISC: &str = "1";
/// The disc total.
pub const DISC_TOTAL: &str = "1";
/// The two composer values. The second value proves that a multi-value field is kept.
pub const COMPOSERS: [&str; 2] = ["Fixture Composer", "Fixture Composer Two"];
/// The original release date.
pub const ORIGINAL_DATE: &str = "1999-05-06";
/// The ISRC.
pub const ISRC: &str = "US-FIX-20-00001";
/// The barcode.
pub const BARCODE: &str = "0123456789012";
/// The catalog number.
pub const CATALOG_NUMBER: &str = "FIX-CAT-001";
/// The record label.
pub const LABEL: &str = "Fixture Label";
/// The copyright line.
pub const COPYRIGHT: &str = "2020 Fixture Records";
/// The lyricist.
pub const LYRICIST: &str = "Fixture Lyricist";
/// The artist sort order.
pub const ARTIST_SORT: &str = "Artist, Fixture";
/// The album artist sort order.
pub const ALBUM_ARTIST_SORT: &str = "Album Artist, Fixture";
/// The media type.
pub const MEDIA: &str = "CD";
/// The script.
pub const SCRIPT: &str = "Latn";
/// The release type.
pub const RELEASE_TYPE: &str = "album";
/// The release status.
pub const RELEASE_STATUS: &str = "official";
/// The release country.
pub const RELEASE_COUNTRY: &str = "US";
/// The original release year. Picard defines it for Vorbis comments only.
pub const ORIGINAL_YEAR: &str = "1999";
/// The six MusicBrainz identifiers.
pub const MUSICBRAINZ_IDS: [(&str, &str); 6] = [
    (
        "MUSICBRAINZ_ARTISTID",
        "11111111-1111-1111-1111-111111111101",
    ),
    (
        "MUSICBRAINZ_ALBUMARTISTID",
        "11111111-1111-1111-1111-111111111102",
    ),
    (
        "MUSICBRAINZ_ALBUMID",
        "11111111-1111-1111-1111-111111111103",
    ),
    (
        "MUSICBRAINZ_RELEASEGROUPID",
        "11111111-1111-1111-1111-111111111104",
    ),
    (
        "MUSICBRAINZ_RELEASETRACKID",
        "11111111-1111-1111-1111-111111111105",
    ),
    ("MUSICBRAINZ_WORKID", "11111111-1111-1111-1111-111111111106"),
];
/// The custom field name. Its value crosses every format by this name.
pub const CUSTOM_NAME: &str = "MYCUSTOM";
/// The custom field value.
pub const CUSTOM_VALUE: &str = "custom one";
/// The portable release-status name: the Vorbis key.
pub const RELEASE_STATUS_NAME: &str = "RELEASESTATUS";
/// The native release-status name Picard uses in ID3 and MP4. Picard writes it with its spaces,
/// so the fixture does too.
pub const RELEASE_STATUS_NATIVE: &str = "MusicBrainz Album Status";
/// The native script name Picard uses in ID3 and MP4.
pub const SCRIPT_NATIVE: &str = "SCRIPT";
/// The native original-year name, which Picard defines for Vorbis comments only.
pub const ORIGINAL_YEAR_NAME: &str = "ORIGINALYEAR";
/// The cover's description.
pub const COVER_DESCRIPTION: &str = "Cover (front)";

/// The three source families.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// An MP3 with an ID3v2 tag.
    Mp3,
    /// An M4A with MP4 atoms.
    M4a,
    /// A FLAC with Vorbis comments.
    Flac,
}

/// The three tag fixtures, in a temporary directory that is removed when the guard drops.
pub struct TagFixtures {
    dir: TempDir,
}

impl TagFixtures {
    /// Build every fixture.
    pub fn build() -> TagFixtures {
        let fixtures = TagFixtures {
            dir: TempDir::new("melomae-tag-fixtures"),
        };
        fixtures.write_all();
        fixtures
    }

    /// The fixture directory.
    pub fn dir(&self) -> &Path {
        self.dir.path()
    }

    /// The path of one fixture source.
    pub fn path(&self, source: Source) -> PathBuf {
        self.dir.join(match source {
            Source::Mp3 => "tags.mp3",
            Source::M4a => "tags.m4a",
            Source::Flac => "tags.flac",
        })
    }

    /// Every source, with its name.
    pub fn sources(&self) -> [(Source, &'static str); 3] {
        [
            (Source::Mp3, "mp3"),
            (Source::M4a, "m4a"),
            (Source::Flac, "flac"),
        ]
    }

    fn write_all(&self) {
        let wav = self.source_wav();
        let cover = self.cover_jpeg();
        let cover_bytes = fs::read(&cover).expect("read the synthesized cover");

        self.mux(
            &wav,
            &cover,
            "tags.mp3",
            &["-c:a", "libmp3lame", "-b:a", "192k"],
            false,
        );
        self.mux(
            &wav,
            &cover,
            "tags.m4a",
            &["-c:a", "aac", "-b:a", "192k"],
            false,
        );
        // The FLAC muxer reads the picture type from the attached stream's comment. The ID3 and MP4
        // muxers write no type from it, and the MP4 muxer turns it into a spurious comment field.
        self.mux(&wav, &cover, "tags.flac", &["-c:a", "flac"], true);

        self.tag_mp3(&cover_bytes);
        self.tag_m4a(&cover_bytes);
        self.tag_flac(&cover_bytes);
    }

    /// A one-second 44.1 kHz stereo sine, synthesized by ffmpeg.
    fn source_wav(&self) -> PathBuf {
        let path = self.dir.join("source.wav");
        run(&[
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=44100:duration=1",
            "-ac",
            "2",
            "-c:a",
            "pcm_f32le",
            &path.display().to_string(),
        ]);
        path
    }

    /// A small JPEG, synthesized by ffmpeg from a color source.
    fn cover_jpeg(&self) -> PathBuf {
        let path = self.dir.join("cover.jpg");
        run(&[
            "-f",
            "lavfi",
            "-i",
            "color=c=red:s=64x64",
            "-frames:v",
            "1",
            &path.display().to_string(),
        ]);
        path
    }

    /// Mux the audio and the cover, and write eight standard fields with ffmpeg's own key names.
    fn mux(&self, wav: &Path, cover: &Path, name: &str, codec: &[&str], describe_picture: bool) {
        let out = self.dir.join(name);
        let mut args: Vec<String> = vec![
            "-i".into(),
            wav.display().to_string(),
            "-i".into(),
            cover.display().to_string(),
            "-map".into(),
            "0:a".into(),
            "-map".into(),
            "1:v".into(),
        ];
        args.extend(codec.iter().map(|value| (*value).to_string()));
        args.extend([
            "-c:v".into(),
            "copy".into(),
            "-disposition:v".into(),
            "attached_pic".into(),
        ]);
        if describe_picture {
            args.push("-metadata:s:v".into());
            args.push(format!("comment={COVER_DESCRIPTION}"));
        }
        args.extend([
            "-metadata".into(),
            format!("title={TITLE}"),
            "-metadata".into(),
            format!("artist={ARTIST}"),
            "-metadata".into(),
            format!("album={ALBUM}"),
            "-metadata".into(),
            format!("album_artist={ALBUM_ARTIST}"),
            "-metadata".into(),
            format!("track={TRACK}/{TRACK_TOTAL}"),
            "-metadata".into(),
            format!("disc={DISC}/{DISC_TOTAL}"),
            "-metadata".into(),
            format!("date={DATE}"),
            "-metadata".into(),
            format!("genre={GENRE}"),
        ]);
        args.push(out.display().to_string());
        let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
        run(&borrowed);
    }

    /// The extra standard fields, in order. A multi-value field repeats its key.
    fn extra_standard() -> Vec<(ItemKey, &'static str)> {
        let mut fields = vec![
            (ItemKey::Composer, COMPOSERS[0]),
            (ItemKey::Composer, COMPOSERS[1]),
            (ItemKey::OriginalReleaseDate, ORIGINAL_DATE),
            (ItemKey::Isrc, ISRC),
            (ItemKey::Barcode, BARCODE),
            (ItemKey::CatalogNumber, CATALOG_NUMBER),
            (ItemKey::Label, LABEL),
            (ItemKey::CopyrightMessage, COPYRIGHT),
            (ItemKey::Lyricist, LYRICIST),
            (ItemKey::TrackArtistSortOrder, ARTIST_SORT),
            (ItemKey::AlbumArtistSortOrder, ALBUM_ARTIST_SORT),
            (ItemKey::OriginalMediaType, MEDIA),
            (ItemKey::Script, SCRIPT),
            (ItemKey::MusicBrainzReleaseType, RELEASE_TYPE),
            (ItemKey::ReleaseCountry, RELEASE_COUNTRY),
        ];
        for (key, value) in MUSICBRAINZ_IDS {
            fields.push((musicbrainz_key(key), value));
        }
        fields
    }

    /// The custom fields that cross every format by name, apart from the release status. The
    /// release status uses the native Picard name of each format, which differs from the portable
    /// name, so each writer adds it separately.
    fn extra_custom() -> [(&'static str, &'static str); 1] {
        [(CUSTOM_NAME, CUSTOM_VALUE)]
    }

    /// The front cover, built from the ffmpeg-made JPEG.
    fn cover(&self, bytes: &[u8]) -> Picture {
        Picture::unchecked(bytes.to_vec())
            .pic_type(PictureType::CoverFront)
            .mime_type(MimeType::Jpeg)
            .description(COVER_DESCRIPTION)
            .build()
    }

    fn tag_mp3(&self, cover_bytes: &[u8]) {
        let path = self.path(Source::Mp3);
        let mut file = fs::File::open(&path).expect("open the MP3");
        let mpeg =
            lofty::mpeg::MpegFile::read_from(&mut file, ParseOptions::new()).expect("read the MP3");
        let mut id3 = mpeg.id3v2().cloned().unwrap_or_else(Id3v2Tag::new);

        // Drop ffmpeg's encoder frame and its picture; the fixture writes its own.
        id3.retain(|frame| frame.id_str() != "TSSE" && frame.id_str() != "APIC");

        let extra = standard_tag(TagType::Id3v2, &Self::extra_standard());
        for frame in Id3v2Tag::from(extra) {
            id3.insert(frame);
        }
        for (name, value) in Self::extra_custom() {
            id3.insert_user_text(name.to_string(), value.to_string());
        }
        // The release status and the script use the Picard native `TXXX` names. Lofty's generic
        // layer has no ID3 frame for either, so the fixture writes them natively.
        id3.insert_user_text(
            RELEASE_STATUS_NATIVE.to_string(),
            RELEASE_STATUS.to_string(),
        );
        id3.insert_user_text(SCRIPT_NATIVE.to_string(), SCRIPT.to_string());
        id3.insert_picture(self.cover(cover_bytes));
        id3.save_to_path(&path, WriteOptions::default())
            .expect("write the MP3 tags");
    }

    fn tag_flac(&self, cover_bytes: &[u8]) {
        let path = self.path(Source::Flac);
        let mut file = fs::File::open(&path).expect("open the FLAC");
        let flac = lofty::flac::FlacFile::read_from(&mut file, ParseOptions::new())
            .expect("read the FLAC");
        let mut comments = flac.vorbis_comments().cloned().unwrap_or_default();

        for key in ["encoder", "ENCODER"] {
            let _ = comments.remove(key).count();
        }
        for (key, value) in Self::extra_standard() {
            if let Some(native) = key.map_key(TagType::VorbisComments) {
                comments.push(native.to_string(), value.to_string());
            }
        }
        for (name, value) in Self::extra_custom() {
            comments.push(name.to_string(), value.to_string());
        }
        comments.push(RELEASE_STATUS_NAME.to_string(), RELEASE_STATUS.to_string());
        comments.push(ORIGINAL_YEAR_NAME.to_string(), ORIGINAL_YEAR.to_string());
        comments
            .insert_picture(self.cover(cover_bytes), None)
            .expect("insert the FLAC picture");
        comments
            .save_to_path(&path, WriteOptions::default())
            .expect("write the FLAC tags");
    }

    fn tag_m4a(&self, cover_bytes: &[u8]) {
        let path = self.path(Source::M4a);
        let mut file = fs::File::open(&path).expect("open the M4A");
        let mp4 =
            lofty::mp4::Mp4File::read_from(&mut file, ParseOptions::new()).expect("read the M4A");
        let mut ilst = mp4.ilst().cloned().unwrap_or_else(Ilst::new);

        let _ = ilst.remove(&AtomIdent::Fourcc(*b"\xa9too"));
        ilst.remove_pictures();
        let extra = standard_tag(TagType::Mp4Ilst, &Self::extra_standard());
        for atom in Ilst::from(extra) {
            ilst.insert(atom);
        }
        for (name, value) in Self::extra_custom() {
            ilst.insert(lofty::mp4::Atom::new(
                AtomIdent::Freeform {
                    mean: "com.apple.iTunes".into(),
                    name: (*name).into(),
                },
                lofty::mp4::AtomData::UTF8(value.to_string()),
            ));
        }
        // The release status uses its native Picard freeform name, not the portable Vorbis key.
        ilst.insert(lofty::mp4::Atom::new(
            AtomIdent::Freeform {
                mean: "com.apple.iTunes".into(),
                name: RELEASE_STATUS_NATIVE.into(),
            },
            lofty::mp4::AtomData::UTF8(RELEASE_STATUS.to_string()),
        ));
        ilst.insert_picture(self.cover(cover_bytes));
        ilst.save_to_path(&path, WriteOptions::default())
            .expect("write the M4A tags");
    }
}

/// A generic tag of `tag_type` that holds the extra standard fields.
fn standard_tag(tag_type: TagType, fields: &[(ItemKey, &'static str)]) -> Tag {
    let mut tag = Tag::new(tag_type);
    for (key, value) in fields {
        tag.push(TagItem::new(*key, ItemValue::Text((*value).to_string())));
    }
    tag
}

/// The lofty item key for a MusicBrainz Vorbis key.
fn musicbrainz_key(name: &str) -> ItemKey {
    match name {
        "MUSICBRAINZ_ARTISTID" => ItemKey::MusicBrainzArtistId,
        "MUSICBRAINZ_ALBUMARTISTID" => ItemKey::MusicBrainzReleaseArtistId,
        "MUSICBRAINZ_ALBUMID" => ItemKey::MusicBrainzReleaseId,
        "MUSICBRAINZ_RELEASEGROUPID" => ItemKey::MusicBrainzReleaseGroupId,
        "MUSICBRAINZ_RELEASETRACKID" => ItemKey::MusicBrainzTrackId,
        other => {
            debug_assert_eq!(other, "MUSICBRAINZ_WORKID");
            ItemKey::MusicBrainzWorkId
        }
    }
}

/// The expected portable manifest of a source: one `(name, value)` pair per value, so a
/// multi-value field repeats its name. Every field the task's Test-first plan lists appears once,
/// except the original year, which Picard defines for a Vorbis source only.
pub fn expected_manifest(source: Source) -> Vec<(&'static str, &'static str)> {
    let mut fields = vec![
        ("TITLE", TITLE),
        ("ARTIST", ARTIST),
        ("ALBUM", ALBUM),
        ("ALBUMARTIST", ALBUM_ARTIST),
        ("TRACKNUMBER", TRACK),
        ("TRACKTOTAL", TRACK_TOTAL),
        ("DISCNUMBER", DISC),
        ("DISCTOTAL", DISC_TOTAL),
        ("DATE", DATE),
        ("ORIGINALDATE", ORIGINAL_DATE),
        ("GENRE", GENRE),
        ("ISRC", ISRC),
        ("BARCODE", BARCODE),
        ("CATALOGNUMBER", CATALOG_NUMBER),
        ("LABEL", LABEL),
        ("COPYRIGHT", COPYRIGHT),
        ("COMPOSER", COMPOSERS[0]),
        ("COMPOSER", COMPOSERS[1]),
        ("LYRICIST", LYRICIST),
        ("ARTISTSORT", ARTIST_SORT),
        ("ALBUMARTISTSORT", ALBUM_ARTIST_SORT),
        ("MEDIA", MEDIA),
        ("SCRIPT", SCRIPT),
        ("RELEASETYPE", RELEASE_TYPE),
        ("RELEASESTATUS", RELEASE_STATUS),
        ("RELEASECOUNTRY", RELEASE_COUNTRY),
        ("MYCUSTOM", CUSTOM_VALUE),
    ];
    for (name, value) in MUSICBRAINZ_IDS {
        fields.push((name, value));
    }
    if source == Source::Flac {
        fields.push(("ORIGINALYEAR", ORIGINAL_YEAR));
    }
    fields
}

/// The kind of native tag a manifest reader reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeKind {
    /// An ID3v2 tag in an MP3.
    Mp3,
    /// An ID3v2 tag in a WAV.
    Wav,
    /// MP4 atoms.
    M4a,
    /// Vorbis comments in a FLAC.
    Flac,
    /// Vorbis comments in an Ogg Opus stream.
    Opus,
}

/// The native tag of one source family.
pub fn source_kind(source: Source) -> NativeKind {
    match source {
        Source::Mp3 => NativeKind::Mp3,
        Source::M4a => NativeKind::M4a,
        Source::Flac => NativeKind::Flac,
    }
}

/// Read a file's native tag and return its portable manifest, one `(name, value)` pair per value.
///
/// This is the independent checker: it reads the format-specific tag, not `read_tags`, so it can
/// catch a field the fixture never wrote or that the generic tag layer loses. A native key with no
/// manifest mapping, such as a cover atom or ffmpeg's encoder field, is ignored.
pub fn native_manifest(path: &Path, kind: NativeKind) -> Vec<(String, String)> {
    native_items(path, kind)
        .into_iter()
        .filter_map(|(key, value)| portable_of(&key).map(|name| (name.to_string(), value)))
        .collect()
}

/// The raw native keys of a file, before the portable mapping. A test uses this to assert a
/// literal target name such as `TPUB` or `TXXX:SCRIPT`.
pub fn native_keys(path: &Path, kind: NativeKind) -> Vec<String> {
    native_items(path, kind)
        .into_iter()
        .map(|(key, _)| key)
        .collect()
}

/// The native tag items of a file, as `(key, value)` pairs. An ID3v2 `TXXX` frame key is
/// `TXXX:<description>`, and an MP4 freeform atom key is `----:com.apple.iTunes:<name>`.
fn native_items(path: &Path, kind: NativeKind) -> Vec<(String, String)> {
    let mut native: Vec<(String, String)> = Vec::new();
    match kind {
        NativeKind::Mp3 | NativeKind::Wav => {
            for frame in read_native_id3(path, kind) {
                match frame {
                    Frame::UserText(frame) => {
                        native.push((
                            format!("TXXX:{}", frame.description),
                            frame.content.to_string(),
                        ));
                    }
                    Frame::Text(frame) => {
                        let id = frame.id().as_str();
                        for value in frame.value.split('\0') {
                            match id {
                                "TRCK" => push_number_pair(
                                    &mut native,
                                    "TRACKNUMBER",
                                    "TRACKTOTAL",
                                    value,
                                ),
                                "TPOS" => {
                                    push_number_pair(&mut native, "DISCNUMBER", "DISCTOTAL", value)
                                }
                                _ => native.push((id.to_string(), value.to_string())),
                            }
                        }
                    }
                    Frame::Timestamp(frame) => {
                        let date = format_timestamp(&frame.timestamp);
                        native.push((frame.id().as_str().to_string(), date));
                    }
                    _ => {}
                }
            }
        }
        NativeKind::M4a => {
            for atom in read_native_ilst(path) {
                match atom.ident() {
                    AtomIdent::Freeform { name, .. } => {
                        for data in atom.data() {
                            if let AtomData::UTF8(text) = data {
                                native.push((
                                    format!("----:com.apple.iTunes:{name}"),
                                    text.to_string(),
                                ));
                            }
                        }
                    }
                    AtomIdent::Fourcc(fourcc) => {
                        if fourcc == b"trkn" || fourcc == b"disk" {
                            let (number, total) = mp4_number_pair(&atom);
                            let (number_name, total_name) = if fourcc == b"trkn" {
                                ("TRACKNUMBER", "TRACKTOTAL")
                            } else {
                                ("DISCNUMBER", "DISCTOTAL")
                            };
                            native.push((number_name.to_string(), number));
                            native.push((total_name.to_string(), total));
                            continue;
                        }
                        let Some(key) = fourcc_key(fourcc) else {
                            continue;
                        };
                        for data in atom.data() {
                            if let AtomData::UTF8(text) = data {
                                native.push((key.to_string(), text.to_string()));
                            }
                        }
                    }
                }
            }
        }
        NativeKind::Flac | NativeKind::Opus => {
            for (key, value) in read_native_vorbis(path, kind) {
                native.push((key, value));
            }
        }
    }
    native
}

/// Split a `3/12` text pair, such as ID3 `TRCK`, into its number and total.
fn push_number_pair(out: &mut Vec<(String, String)>, number: &str, total: &str, value: &str) {
    let mut parts = value.splitn(2, '/');
    out.push((
        number.to_string(),
        parts.next().unwrap_or_default().to_string(),
    ));
    if let Some(rest) = parts.next() {
        out.push((total.to_string(), rest.to_string()));
    }
}

/// The number and total of a `trkn` or `disk` atom. The pair is two big-endian `u16` values after
/// a two-byte reserved field.
fn mp4_number_pair(atom: &lofty::mp4::Atom<'static>) -> (String, String) {
    for data in atom.data() {
        if let AtomData::Unknown { data, .. } = data {
            if data.len() >= 6 {
                let number = u16::from_be_bytes([data[2], data[3]]);
                let total = u16::from_be_bytes([data[4], data[5]]);
                return (number.to_string(), total.to_string());
            }
        }
    }
    (String::new(), String::new())
}

/// The manifest name of an MP4 four-character atom, or `None` for a field that is not manifest.
fn fourcc_key(fourcc: &[u8; 4]) -> Option<&'static str> {
    Some(match fourcc {
        b"\xa9nam" => "TITLE",
        b"\xa9ART" => "ARTIST",
        b"\xa9alb" => "ALBUM",
        b"aART" => "ALBUMARTIST",
        b"\xa9day" => "DATE",
        b"\xa9gen" => "GENRE",
        b"\xa9wrt" => "COMPOSER",
        b"cprt" => "COPYRIGHT",
        b"soar" => "ARTISTSORT",
        b"soaa" => "ALBUMARTISTSORT",
        _ => return None,
    })
}

/// The portable manifest name of a native key, or `None` when the key is not a manifest field.
///
/// The lookup strips a `TXXX:` or `----:com.apple.iTunes:` prefix, then ignores case, spaces, and
/// underscores, so `MUSICBRAINZ_ARTISTID`, `MusicBrainzArtistId`, and `MusicBrainz Artist Id` all
/// resolve to the same name.
fn portable_of(key: &str) -> Option<&'static str> {
    let name = key.rsplit(':').next().unwrap_or(key);
    let normalized: String = name
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '_')
        .flat_map(char::to_uppercase)
        .collect();
    Some(match normalized.as_str() {
        "TITLE" | "TIT2" => "TITLE",
        "ARTIST" | "TPE1" => "ARTIST",
        "ALBUM" | "TALB" => "ALBUM",
        "ALBUMARTIST" | "TPE2" => "ALBUMARTIST",
        "TRACKNUMBER" => "TRACKNUMBER",
        "TRACKTOTAL" => "TRACKTOTAL",
        "DISCNUMBER" => "DISCNUMBER",
        "DISCTOTAL" => "DISCTOTAL",
        "DATE" | "TDRC" => "DATE",
        "ORIGINALDATE" | "TDOR" | "TORY" => "ORIGINALDATE",
        "ORIGINALYEAR" => "ORIGINALYEAR",
        "GENRE" | "TCON" => "GENRE",
        "ISRC" | "TSRC" => "ISRC",
        "BARCODE" => "BARCODE",
        "CATALOGNUMBER" => "CATALOGNUMBER",
        "LABEL" | "TPUB" => "LABEL",
        "COPYRIGHT" | "TCOP" => "COPYRIGHT",
        "COMPOSER" | "TCOM" => "COMPOSER",
        "LYRICIST" | "TEXT" => "LYRICIST",
        "ARTISTSORT" | "TSOP" => "ARTISTSORT",
        "ALBUMARTISTSORT" | "TSO2" => "ALBUMARTISTSORT",
        "MEDIA" | "TMED" => "MEDIA",
        "SCRIPT" => "SCRIPT",
        "RELEASETYPE" | "MUSICBRAINZALBUMTYPE" => "RELEASETYPE",
        "RELEASESTATUS" | "MUSICBRAINZALBUMSTATUS" => "RELEASESTATUS",
        "RELEASECOUNTRY" | "MUSICBRAINZALBUMRELEASECOUNTRY" => "RELEASECOUNTRY",
        "MUSICBRAINZARTISTID" => "MUSICBRAINZ_ARTISTID",
        "MUSICBRAINZALBUMARTISTID" => "MUSICBRAINZ_ALBUMARTISTID",
        "MUSICBRAINZALBUMID" => "MUSICBRAINZ_ALBUMID",
        "MUSICBRAINZRELEASEGROUPID" => "MUSICBRAINZ_RELEASEGROUPID",
        "MUSICBRAINZRELEASETRACKID" => "MUSICBRAINZ_RELEASETRACKID",
        "MUSICBRAINZWORKID" => "MUSICBRAINZ_WORKID",
        "MYCUSTOM" => "MYCUSTOM",
        _ => return None,
    })
}

/// Format an ID3 timestamp the way the other formats write a date.
fn format_timestamp(timestamp: &lofty::tag::items::Timestamp) -> String {
    let mut date = format!("{:04}", timestamp.year);
    if let Some(month) = timestamp.month {
        date.push_str(&format!("-{month:02}"));
    }
    if let Some(day) = timestamp.day {
        date.push_str(&format!("-{day:02}"));
    }
    date
}

/// The ID3v2 tag of an MP3 or a WAV output.
fn read_native_id3(path: &Path, kind: NativeKind) -> Id3v2Tag {
    let mut file = fs::File::open(path).expect("open the ID3 file");
    match kind {
        NativeKind::Mp3 => lofty::mpeg::MpegFile::read_from(&mut file, ParseOptions::new())
            .expect("read the MP3")
            .id3v2()
            .cloned()
            .unwrap_or_default(),
        NativeKind::Wav => lofty::iff::wav::WavFile::read_from(&mut file, ParseOptions::new())
            .expect("read the WAV")
            .id3v2()
            .cloned()
            .unwrap_or_default(),
        _ => unreachable!("only MP3 and WAV carry an ID3v2 tag here"),
    }
}

/// The `ilst` of an M4A output.
fn read_native_ilst(path: &Path) -> Ilst {
    let mut file = fs::File::open(path).expect("open the M4A");
    lofty::mp4::Mp4File::read_from(&mut file, ParseOptions::new())
        .expect("read the M4A")
        .ilst()
        .cloned()
        .unwrap_or_else(Ilst::new)
}

/// The Vorbis comments of a FLAC or Opus output.
fn read_native_vorbis(path: &Path, kind: NativeKind) -> Vec<(String, String)> {
    let mut file = fs::File::open(path).expect("open the Vorbis file");
    let options = ParseOptions::new();
    let comments = match kind {
        NativeKind::Flac => lofty::flac::FlacFile::read_from(&mut file, options)
            .expect("read the FLAC")
            .vorbis_comments()
            .cloned()
            .unwrap_or_default(),
        NativeKind::Opus => lofty::ogg::OpusFile::read_from(&mut file, options)
            .expect("read the Opus")
            .vorbis_comments()
            .clone(),
        _ => unreachable!("only FLAC and Opus carry Vorbis comments here"),
    };
    comments
        .items()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

/// Run ffmpeg, failing the test when it exits non-zero.
fn run(args: &[&str]) {
    let output = Command::new(ffmpeg())
        .args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(args)
        .output()
        .expect("run the pinned ffmpeg");
    assert!(
        output.status.success(),
        "ffmpeg {args:?} failed with {}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}
