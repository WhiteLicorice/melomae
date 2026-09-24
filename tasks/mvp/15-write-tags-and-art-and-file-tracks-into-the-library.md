# 15 — Write tags and art and file tracks into the library

**Status:** TODO
**Phase:** 3 — Library and pipeline
**Depends on:** 06, 09, 10
**SRS:** §8
**Stack:** §F
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Write canonical MusicBrainz tags and the front cover into each output file, and put the file at its library path.

## Evidence

- The owner's old pipeline copied seven tags into WAV ID3 frames (`C:\Lab\Apollo\metadata_utils.py:10-18`). Melomae writes the full MusicBrainz set.
- The owner's library holds names such as `Florida!!!` and `Habits (Stay High)` (`C:\Lab\Apollo\run_batch.py` docstring).

## Scope

1. Use lofty. Write Vorbis comments for FLAC and Opus. Write ID3 for WAV.
2. Follow the MusicBrainz Picard tag mapping. Confirm it against the live Picard documentation at execution time and record the source and date:
   - `MUSICBRAINZ_TRACKID` holds the recording MBID.
   - `MUSICBRAINZ_RELEASETRACKID`, `MUSICBRAINZ_ALBUMID`, `MUSICBRAINZ_RELEASEGROUPID`, `MUSICBRAINZ_ARTISTID`, and `MUSICBRAINZ_ALBUMARTISTID` hold their IDs.
   - Also write title, artist, album artist, album, `ISRC`, track number and total, disc number and total, `DATE`, `ORIGINALDATE`, and `GENRE`.
3. Embed the front cover from task 10.
4. Build the path as `{albumartist}/{album}/{track:02} {title}.{ext}`. A multi-disc release uses `{disc}-{track:02} {title}.{ext}`.
5. Make each name safe on Windows and Linux:
   - replace `<>:"/\|?*` and control characters
   - trim trailing dots and spaces
   - rename Windows reserved names such as `CON`, `NUL`, and `COM1`
   - cap each path component at 180 bytes
6. On a name collision with a different recording, append ` (2)`, then ` (3)`.

## Out of scope

- Deduplication (task 16)

## Test-first plan

1. A tag round trip for each format reads back every written tag.
2. Sanitizer table tests: `AC/DC`, `CON`, `Florida!!!`, `Habits (Stay High)`, and `Sigur Rós`.
3. A collision test produces ` (2)`.

Run the tests. They fail because the writer does not exist.

## Acceptance criteria

- [ ] A third-party player shows the tags and the cover for one file of each format. Name the player in `Outcome`.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core library_writer
pnpm verify
```
