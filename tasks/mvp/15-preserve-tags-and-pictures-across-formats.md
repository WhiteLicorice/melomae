# 15 — Preserve tags and pictures across formats

**Status:** DONE
**Phase:** 2 — Metadata, triage, and provenance
**Depends on:** 06
**SRS:** §5, §12
**Stack:** §F
**Assigned to:** Agent
**Started:** 4000238 (task/15)
**Outcome:** Done. `cargo test -p melomae-core tag_copy` runs 6 unit tests and 5 integration tests, all green. The red run stubbed `read_tags` and `write_into` and failed three e2e tests for the expected reason. A new `tag_copy` module reads every tag and picture. It keeps the standard fields in lofty's generic model and the custom fields by name (ID3 `TXXX`, a Vorbis key, an MP4 freeform atom). It writes ID3 for WAV and Vorbis comments for FLAC and Opus. `encode_with_tags` writes the tags into the temporary file before the rename, so an output never appears without its tags. `encode` is unchanged and every task 06 test stays green. A field the target cannot hold is named, never dropped: the fixture's script field is reported for a WAV output. lofty 0.25.4, MIT OR Apache-2.0, compatible with GPL-3.0-or-later. The Picard tag mapping came from https://picard-docs.musicbrainz.org/en/latest/appendices/tag_mapping.html on 2026-10-11. lofty has no custom `ItemKey`, so the copier uses the format-specific API for custom fields (scope item 8). The third-party player check is open for the author. The outputs to open come from `e2e_tag_copy_outputs_read_back_in_ffprobe`. Gate: `pnpm verify` exit 0. Board 55 records, tasks unittest 9, Vitest 1, cargo test 133, pytest 13.

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

## Verify

```powershell
cargo test -p melomae-core tag_copy
pnpm verify
```
