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
use lofty::id3::v2::Id3v2Tag;
use lofty::mp4::{AtomIdent, Ilst};
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
/// The release status. This has no standard field in any output format.
pub const RELEASE_STATUS: &str = "official";
/// The release country.
pub const RELEASE_COUNTRY: &str = "US";
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
/// The custom release-status field name, which is how the field crosses every format.
pub const RELEASE_STATUS_NAME: &str = "RELEASESTATUS";
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

    /// The custom fields, in order.
    fn extra_custom() -> [(String, Vec<&'static str>); 2] {
        [
            (RELEASE_STATUS_NAME.to_string(), vec![RELEASE_STATUS]),
            (CUSTOM_NAME.to_string(), vec![CUSTOM_VALUE]),
        ]
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
        for (name, values) in Self::extra_custom() {
            id3.insert_user_text(name, values.join("\0"));
        }
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
        for (name, values) in Self::extra_custom() {
            for value in values {
                comments.push(name.clone(), value.to_string());
            }
        }
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
        for (name, values) in Self::extra_custom() {
            for value in values {
                ilst.insert(lofty::mp4::Atom::new(
                    AtomIdent::Freeform {
                        mean: "com.apple.iTunes".into(),
                        name: name.clone().into(),
                    },
                    lofty::mp4::AtomData::UTF8(value.to_string()),
                ));
            }
        }
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
