# 30 — Define the stream source interface

**Status:** TODO
**Phase:** 2 — Metadata and acquisition
**Depends on:** 00, 03
**SRS:** §5, §6
**Stack:** §B, §E
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Put every stream source behind one interface. A new source, such as Bandcamp or SoundCloud, must then change only its own module and one registry line.

## Evidence

- The owner made source flexibility a core requirement on 2026-09-24. The Bandcamp and SoundCloud wiring stays in the backlog (B-009, B-010).
- yt-dlp supports Bandcamp and SoundCloud (`supportedsites.md`, checked 2026-09-24). A new source can reuse the managed yt-dlp binary from task 11.
- Some sources deliver lossless audio. The Bandcamp free-download page offers other encodings (`yt_dlp/extractor/bandcamp.py:223-280`). SoundCloud offers the "wav original" with authentication (`yt_dlp/extractor/soundcloud.py:597`). The interface must report this so SRS §6.10 can skip restore.

## Scope

1. Add the module `melomae-core::source` with the trait `StreamSource`:
   - `id()` and `display_name()`
   - `required_tools()`: the external tools the source needs. Task 11 installs them.
   - `search(&TrackQuery) -> Vec<Candidate>`
   - `claims_url(&Url) -> bool`
   - `candidate_from_url(&Url) -> Candidate`
   - `fetch(&Candidate, dest, progress, cancel) -> FetchedStream`
2. Define the neutral types:
   - `TrackQuery`: title, artist credit, album, duration, ISRC, and MusicBrainz IDs.
   - `Candidate`: source ID, source URL, title, artist, duration, `official_audio: Option<bool>`, and a thumbnail URL.
   - `StreamInfo`: codec, container, bitrate, sample rate, and `lossless`.
   - `FetchedStream`: a file path and a `StreamInfo`.
3. Each source chooses its own stream format. The interface only requires the source to report what it fetched in `StreamInfo`. A source never re-encodes.
4. Add a `SourceRegistry`. It holds the sources in priority order and routes a pasted URL to the source that claims it. An unclaimed URL returns a typed error.
5. Add a reusable contract suite, `source_contract::run(factory)`. Every source implementation must pass it:
   - `search` results carry the source's own ID.
   - `fetch` writes a file that the task 03 decoder opens.
   - `StreamInfo` matches the decoded codec.
   - A cancel leaves no file.
   - `claims_url` accepts the source's own candidate URLs.
6. Add `FakeSource` and `LocalFileSource` behind `cfg(any(test, feature = "test-sources"))`. Task 17 uses `LocalFileSource` for its real end-to-end run. The app does not register either one.
7. Add a boundary test. It fails if any file under `crates/melomae-core/src/` outside `source/youtube/` names `yt-dlp`, `yt_dlp`, or `ytdlp`.

## Out of scope

- The YouTube Music implementation (tasks 12 and 13)
- The Bandcamp and SoundCloud sources (B-009, B-010)
- A source-priority UI, or settings for more than one source

## Test-first plan

1. Run the contract suite on `FakeSource` and `LocalFileSource`.
2. Write the registry routing tests: a claimed URL goes to its source, and an unclaimed URL returns the typed error.
3. Write the boundary test.

Run the tests. They fail because the module does not exist.

## Acceptance criteria

- [ ] Both test sources pass the contract suite.
- [ ] The registry routes and rejects URLs as specified.
- [ ] The boundary test passes.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core source
pnpm verify
```
