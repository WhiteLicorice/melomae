# 15 — Preserve tags and pictures across formats

**Status:** DONE
**Phase:** 2 — Metadata, triage, and provenance
**Depends on:** 06
**SRS:** §5, §12
**Stack:** §F
**Assigned to:** Agent
**Started:** 4000238 (task/15)
**Outcome:** Done. `cargo test -p melomae-core` runs 7 tag_copy unit tests and 15 tag_copy integration tests, all green. The round-1 red run failed seven new tests for the reviewer's reasons: an ID3 `TPUB` read as `PUBLISHER`, the release status read as a custom field, the script reported for a WAV output, and the original year lost into the original date. A new `tag_copy` module reads every tag and picture. It keeps the standard fields lofty maps in its generic model, and the custom fields by name (ID3 `TXXX`, a Vorbis key, an MP4 freeform atom). An explicit Picard override table carries the fields lofty's generic layer mis-names or merges: ID3 `TPUB` to Vorbis `LABEL` and back, the release status to `RELEASESTATUS`, the script into a WAV `TXXX:SCRIPT` frame, and the original year apart from the original date (reported for WAV, which has no Picard ID3 name for it). It writes ID3 for WAV and Vorbis comments for FLAC and Opus. `encode_with_tags` writes the tags into the temporary file before the rename, so an output never appears without its tags. `encode` is unchanged and every task 06 test stays green. A field the target cannot hold is named, never dropped. The fixture records an explicit source manifest, checked against a native reader before the copier runs, with the script written natively in the MP3 and the original year in the FLAC. ID3v2.3 slash text stays one value; the author chose that on 2026-10-11. lofty 0.25.4, MIT OR Apache-2.0, compatible with GPL-3.0-or-later. The Picard tag mapping came from https://picard-docs.musicbrainz.org/en/latest/appendices/tag_mapping.html, checked 2026-10-11. The ID3v2 and MP4 names of the MusicBrainz fields carry their spaces, as the live page shows (`TXXX:MusicBrainz Album Status`), so the copier writes them that way. lofty has no custom `ItemKey`, so the copier uses the format-specific API for custom fields (scope item 8). The third-party player check is open for the author. The outputs to open come from `e2e_tag_copy_outputs_read_back_in_ffprobe`. The fix-round-2 review's only blocking finding is fixed: lofty writes the Vorbis key `ENCODING` for `EncoderSettings`, but Picard names it `ENCODERSETTINGS`. A `PicardMapping.key` now overrides the `EncoderSettings` item, so the reader names it `ENCODERSETTINGS` and the Vorbis writer emits that literal key in FLAC and Opus, from both an ID3 `TSSE` source and a Vorbis `ENCODERSETTINGS` source; two Vorbis values stay two. A WAV output keeps its `TSSE` frame, and `EncoderSoftware` (`ENCODER`) is separate and unchanged. The other override rows were checked against the live Picard page for the same pattern and none mismatches (`SCRATCH\evidence-15\picard-tag-mapping.md`). Gate: `pnpm verify` exit 0 at `39b554f`. Board 55 records, tasks unittest 9, Vitest 1, cargo test 144, pytest 13. Red-first evidence: `SCRATCH\evidence-15\red-round1.txt` (round 1) and `SCRATCH\evidence-15\red-round2.txt` (round 2); green: `SCRATCH\evidence-15\green-round1.txt` and `SCRATCH\evidence-15\green-round2.txt`.

## Goal

Copy every tag and picture from a source file to its output, in any supported output format, and report any field that can't be carried.

## Evidence

- One competitor kept 2 of 30 tags and changed the cover type from "Cover (front)" to "Other". Another kept no tags and no cover, and renamed the file (`docs/evidence/2026-10-10-pivot.md`). This task prevents both failures.
- The owner's old pipeline copied seven tags into WAV ID3 frames (`C:\Lab\Apollo\metadata_utils.py:10-18`). Melomae copies every field.

## Scope

1. Read every tag and picture from the source with lofty: ID3 from MP3, MP4 atoms from M4A, and Vorbis comments from FLAC, Opus, and Ogg.
2. Write Vorbis comments for FLAC and Opus output. Write ID3 for WAV output.
3. Map standard fields between formats with the MusicBrainz Picard tag mapping. Confirm it against the live Picard documentation at execution time. Record the source URL and the date in `Outcome`.
4. Keep custom fields. An ID3 `TXXX:NAME` becomes the Vorbis key `NAME`, and the reverse. An MP4 freeform atom keeps its name.
5. Keep every value of a multi-value field.
6. Copy every picture with its picture type, description, and MIME type.
7. Return a list of the fields that the target format can't hold. Never drop a field without listing it.
8. Confirm how lofty exposes custom fields, multi-value fields, and picture types. Use the format-specific API where the generic layer loses data. Record the result in `Outcome`.

## Out of scope

- The provenance field (task 31)
- Any online lookup (SRS §5.6)

## Test-first plan

1. Add `tools/fixtures/make_tag_fixture.py`. It is dev-only. It builds a synthetic MP3 from `C:\Lab\Apollo\asserts\input_wav.wav` and writes these fields: title, artist, album, album artist, track 3/12, disc 1/1, date, original date, original year, genre, ISRC, barcode, catalog number, label, copyright, composer with two values, lyricist, artist sort, album artist sort, media, script, release type, release status, release country, the six MusicBrainz IDs, and a JPEG front cover. Use made-up values. Never commit commercial audio or real artwork.
2. For each output format (FLAC, WAV, Opus), a test copies the fixture tags and reads back every field and the front cover with its type.
3. A test with a field that the target can't hold finds it in the returned list.

Run the tests. They fail because the copier does not exist.

## Acceptance criteria

- [x] Every fixture field and the front cover survive in all three formats, or appear in the returned list.
- [ ] A third-party player shows the tags and the cover for one file of each format. Name the player in `Outcome`.
- [x] The new tests failed first for the expected reason.
- [x] `pnpm verify` passes.

## Decision (author, 2026-10-11)

ID3v2.3 stores several artists or composers in one text frame separated by `/` (ID3v2.3 §4.2.1). Splitting on `/` also splits a single name that contains one, such as "AC/DC". The author chose option 1: keep the v2.3 text verbatim as one value, so nothing changes the user's text. `id3v23_slash_text_stays_one_value_pending_author` pins that behavior. Option 3, splitting only when another field in the same file confirms the count, goes to the backlog; the orchestrator files it.

## Verify

```powershell
cargo test -p melomae-core tag_copy
pnpm verify
```
