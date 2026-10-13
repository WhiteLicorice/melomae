# 15 — Preserve tags and pictures across formats

**Status:** DONE
**Phase:** 2 — Metadata, triage, and provenance
**Depends on:** 06
**SRS:** §5, §12
**Stack:** §F
**Assigned to:** Agent
**Started:** 4000238 (task/15)
**Outcome:** Done (fix round 6). `cargo test -p melomae-core` runs 7 `tag_copy` unit tests and 38 `tag_copy` integration tests, all green. `tag_copy::read_tags` reads every tag and picture; `write_into` writes ID3 for WAV and Vorbis comments for FLAC and Opus; `encode_with_tags` writes the tags into the temporary file before the rename, so an output never appears without its tags, and `encode` is unchanged. Every value of a field survives into every output **or is named in the returned list**; the copier no longer claims that every value survives. The write check backs that report for the source-visible portable text values: after the tags are written into the temporary file, `read_written_tags` reads them back and `write_losses` compares the source with the output in the portable key space the copier uses, counting multiplicity, and `encode_with_tags` adds every field that lost a value or is missing to the returned list. The check compares portable text values only, so it does not compare pictures, it excludes carried frames, it does not see a value the copier adds, and it cannot see a collapse the reader makes on both sides. A field the copier deliberately transforms is compared in canonical form: the movement pair joins into one `MVIN` token, the license row uses `WCOP` or `TXXX:LICENSE` as one `LICENSE`, and a date stays one value. An override field's `TXXX` values are grouped by their exact native description and written as one null-separated frame per description (`ARTISTS`, `WRITER`, `WORK`); the record label is one null-separated `TPUB` frame; a custom and a standard representation of one description are combined before writing. `WEBSITE` in a WAV output keeps the first URL in one `WOAR` frame, and the write check names `WEBSITE`, because lofty's `Id3v2Tag` cannot hold two frames with the same id: `UrlLinkFrame` equality and hash use only `header.id` (`lofty-0.25.4/src/id3/v2/items/url_link_frame.rs:18-28`) and the only frame-adding call, `FrameList::push`, is crate private (`lofty-0.25.4/src/id3/v2/frame/list.rs:264`). An MP3 source whose repeated `WOAR` frames lofty collapses on read is detected by a small raw frame-header count, so `WEBSITE` is named there too. lofty's generic mapping corrects, but does not equal, Picard, so an explicit override table keyed by `ItemKey` carries the fields lofty mis-names, merges, or drops: `LABEL`, `LYRICS`, `KEY`, `BPM`, `COMPOSERSORT`, `WRITER`, `ARTISTS`, `WORK`, `ORIGINALFILENAME`, `ENCODERSETTINGS`, the MusicBrainz composer, disc, original-artist, original-release, and TRM IDs, and MusicIP PUID. The live Picard page has 85 rows with both an ID3v2 and a Vorbis entry; `picard_tag_mapping_audit_holds_for_every_dual_format_row` covers 65, and dedicated Group A, Group B, and fixture tests cover the rest, so no single test exercises every live row. The ruled unclear rows are resolved: `WOAR` to `WEBSITE`, `WCOP` and `TXXX:LICENSE` to `LICENSE`, `MVIN` to `MOVEMENT`/`MOVEMENTTOTAL` with a malformed value reported, the MusicBrainz `UFID` to `MUSICBRAINZ_TRACKID` with a foreign UFID preserved and named, and an empty-description `COMM` to `COMMENT`. Two or more license values become one null-separated `TXXX:LICENSE` frame. A native ID3v2 frame the generic model cannot represent is carried verbatim into a WAV output and named for a Vorbis target. The Group B rows keep that no-loss rule; their full mapping and repeated `WOAR` frames in WAV are backlog tickets. The fix-round-5 gate ran once at this head: `pnpm verify` exit 0, board 72 records, tasks unittest 9, Vitest 1, cargo 236 passed and 3 ignored, pytest 45, licenses ok, notices current. Rebased onto `9142778` after task 07 merged: `encode`, `encode_with_tags`, and `encode_with_cancel` now route through one transaction that writes the audio, checks the cancel flag, writes the tags, runs the write check, checks the cancel flag once more, and renames, and every cancel or error before the rename removes the temporary file; the three signatures are unchanged. The `tag_copy` test includes the tag fixtures directly, so the shared `tests/setup` module the CLI tests reuse stays free of lofty. The post-rebase gate ran once at this head: `pnpm verify` exit 0, board 77 records, tasks unittest 9, Vitest 1, cargo 268 passed and 4 ignored, pytest 45, licenses ok, notices current. The fixture's native fallbacks are `tests/setup/tag_fixtures.rs:267`, `:307`, `:336`, and `:364`. Fix round 6 groups `TXXX` values by their exact native description in `push_user_text`, so distinct custom names (`MY_FIELD`, `MYFIELD`, `MY-FIELD`, `MY FIELD`, and case-only pairs) no longer merge in a WAV output, and removes the unused `TagFixtures::dir` helper so `cargo clippy -p melomae-core --all-targets -- -D warnings` passes. The fix-round-6 gate ran once at this head: `pnpm verify` exit 0, board 77 records, tasks unittest 9, Vitest 1, cargo 270 passed and 4 ignored, pytest 45, licenses ok, notices current. Open for the author: acceptance item 2, the third-party player check; the files to open come from `e2e_tag_copy_outputs_read_back_in_ffprobe`.

## Merge review

- Executors: `deepseek-v4.1-flash` (high) in CommandCode, a fresh session for each round: exec15 (round 0), exec15b (fix round 1), exec15c (fix round 2), exec15 (fix round 3, 2026-10-13), exec15d (fix round 4), exec15e (fix round 5 and the rebase), exec15f (fix round 6). Reviewers: seven fresh Codex `gpt-6.1-sol` (medium) runs, adversarial.
- Author ruling (2026-10-11): ID3v2.3 slash text stays one value (option 1). Option 3 is backlog B-049.
- Review 1: FAIL. Missing Picard mappings (TPUB, RELEASESTATUS, SCRIPT in WAV), ORIGINALYEAR merged into ORIGINALDATE, ID3v2.3 slash values, and fixture gaps. Fix round 1 repaired the mappings, kept ORIGINALYEAR, and widened the fixture.
- Review 2 on `7a97913`: FAIL. Encoder settings used the wrong Vorbis identity. Fix round 2 corrected it and audited the five override rows.
- Review 3 on `4341855`: FAIL. Three Picard mappings were wrong (lyrics, key, and BPM and composer sort for Vorbis). Fix round 3 corrected them, added a table audit test, and, under the orchestrator ruling in `ruling-15-round3.md`, mapped the Group A fields (recording ID, website, license, movement, plain comment). Group B (TIPL roles, TMCL, POPM, fingerprint) gets a no-silent-loss test only.
- Review 4 on `351b7a2`: FAIL. LICENSE lost values in WAV. Fix round 4 writes two or more license values into one null-separated `TXXX:LICENSE` frame and added six omitted Picard rows to the audit.
- Review 5 on `5ea2e69`: FAIL. Repeated override values replaced each other in WAV (ARTISTS, WRITER, WORK), extra labels were dropped, and WEBSITE lost URLs. Fix round 5 groups the override values, writes every label, and adds the write check: after the tags are written into the temporary file and before the rename, the encoder reads them back and lists every field that lost a value. Orchestrator ruling (2026-10-14): lofty 0.25.4 cannot hold two `WOAR` frames in one tag (`src/id3/v2/items/url_link_frame.rs:18-28`, `src/id3/v2/frame/list.rs:264`), so WAV keeps the first URL and lists WEBSITE.
- Rebase on `master` `9142778` (task 07 merged): the encoder transaction now writes the audio, checks cancel, writes the tags and runs the write check, checks cancel again, and renames. Gate at `a02bd5d`: exit 0, board 77, cargo 268 passed and 4 ignored.
- Review 6 on `f0adbe5`: FAIL. The new grouping merged distinct custom names (`MY_FIELD`, `MYFIELD`, `MY-FIELD`, `MY FIELD`) in WAV, and `cargo clippy --all-targets -D warnings` failed on an unused fixture helper. `pnpm verify` runs Clippy without `--all-targets`, so the gate did not catch it. Fix round 6 groups by the exact description and removes the helper.
- Gate at `81f3680`: `pnpm verify` exit 0, board 77, unittest 9, Vitest 1, cargo 270 passed and 4 ignored, pytest 45, licenses ok, notices current. `86f944b` changes only this record.
- Review 7 on `86f944b`: PASS. Exact grouping keeps `MY_FIELD`, `MYFIELD`, `MY-FIELD`, `MY FIELD`, `MixedName`, and `mixedname` apart in all three outputs, and the seven round-5 probes still pass. Both `--all-targets` Clippy commands pass. The reviewer re-ran the round-6 tests and Clippy at the test commit `0edeb13` and saw them fail first. Core suite 241 passed and 3 ignored.
- Integration: fast-forward of `master` from `9142778` to `86f944b`. The merged tree equals the reviewed tree. `master` did not move after the gate.
- Review 6 also found that the CLI test `e2e_restore_wav_matches_golden` fails on `master` when free memory gives a chunk shorter than the 18 s input (17 s gave max abs 0.671). Task 15 does not change that path. MVP task 36 holds it.
- Not confirmed: the player check (acceptance item 2) stays open for the author.
- Backlog filed: B-042 to B-050.
- CI ran locally only.

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
