# 15 — Preserve tags and pictures across formats

**Status:** DONE
**Phase:** 2 — Metadata, triage, and provenance
**Depends on:** 06
**SRS:** §5, §12
**Stack:** §F
**Assigned to:** Agent
**Started:** 4000238 (task/15)
**Outcome:** Done (fix round 4). `cargo test -p melomae-core` runs 7 `tag_copy` unit tests and 28 `tag_copy` integration tests, all green. `tag_copy::read_tags` reads every tag and picture; `write_into` writes ID3 for WAV and Vorbis comments for FLAC and Opus; `encode_with_tags` writes the tags into the temporary file before the rename, so an output never appears without its tags, and `encode` is unchanged. Lofty's generic mapping follows Picard closely but is not identical to it, so an explicit override table, keyed by `ItemKey`, carries every field lofty's generic layer mis-names, merges, or drops: `LABEL`, `LYRICS`, `KEY`, `BPM`, `COMPOSERSORT`, `WRITER`, `ARTISTS`, `WORK`, `ORIGINALFILENAME`, `ENCODERSETTINGS`, the MusicBrainz composer, disc, original-artist, original-release, and TRM IDs, and MusicIP PUID. The live Picard page has 85 rows with both an ID3v2 and a Vorbis entry; the table-driven test `picard_tag_mapping_audit_holds_for_every_dual_format_row` covers 65 of them, and dedicated tests cover the rest (one test per Group A row, one Group B no-loss test, and the three-source fixture matrix), so no single test exercises every live row. The ruled unclear rows are resolved: `WOAR` to `WEBSITE`, `WCOP` and `TXXX:LICENSE` to `LICENSE`, `MVIN` to `MOVEMENT`/`MOVEMENTTOTAL` with a malformed value reported, the MusicBrainz `UFID` to `MUSICBRAINZ_TRACKID` with a foreign UFID preserved and named, and an empty-description `COMM` to `COMMENT`. The license row keeps every value: a single URL writes the ID3 `WCOP` frame, and one null-separated `TXXX:LICENSE` frame carries every other case, so a multi-value license set survives into WAV. A native ID3v2 frame the generic model cannot represent is carried verbatim into a WAV output and named for a Vorbis target, so no field is dropped or renamed in silence. The Group B rows (`TIPL`/`IPLS` roles, `TMCL` performer, `POPM` rating, MusicIP fingerprint) keep that no-loss rule; their full mapping is a backlog ticket. The fixture writes eight standard fields with the pinned ffmpeg and the rest with lofty's format-specific API: native fixture fallbacks are `crates/melomae-core/tests/setup/tag_fixtures.rs:272` (the extra standard fields), `:312` (the MP3 tag), `:341` (the FLAC tag), and `:369` (the M4A tag). The Picard mapping came from https://picard-docs.musicbrainz.org/en/latest/_static/MusicBrainz_Picard_Tag_Map.html, checked 2026-10-13. lofty 0.25.4, MIT OR Apache-2.0, compatible with GPL-3.0-or-later, passes `cargo deny` licenses. The third-party player check (acceptance item 2) is open for the author; the outputs to open come from `e2e_tag_copy_outputs_read_back_in_ffprobe`. Gate at head 60ba91e: `pnpm verify` exit 0, board 72 records, tasks unittest 9, Vitest 1, `cargo test --workspace` 228 passed and 3 ignored, pytest 45, licenses ok, notices current. (The fix-round-3 gate at 441ffab is 226 passed and 3 ignored, not 225.)

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
