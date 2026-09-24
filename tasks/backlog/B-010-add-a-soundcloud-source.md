# B-010 — Add a SoundCloud source

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 30, 17

## Impact

Some artists, remixes, and DJ sets exist only on SoundCloud.

## Evidence

Checked on 2026-09-24:

- yt-dlp lists the extractors `soundcloud`, `soundcloud:set`, `soundcloud:playlist`, and `soundcloud:search` with the `scsearch:` prefix (`supportedsites.md`). The managed yt-dlp binary from task 11 can serve this source.
- The default SoundCloud formats are `http_aac`, `hls_aac`, `http_opus`, `hls_opus`, `http_mp3`, and `hls_mp3` (`yt_dlp/extractor/soundcloud.py:109`).
- AAC reaches 256 kb/s for premium accounts (`soundcloud.py:355`).
- The extractor's comments note "wav original available with auth" (`soundcloud.py:597`, `:630`).

## Why this does not block MVP

The owner put real source wiring in the backlog on 2026-09-24. The MVP proves the interface with YouTube Music and the task 30 test sources.

## Expected behavior

1. `SoundCloudSource` implements `StreamSource` in `source/soundcloud/`. It declares yt-dlp in `required_tools()`.
2. It searches through `scsearch:` and maps results to `Candidate`. An upload from the artist's own account maps to `official_audio = true`.
3. `fetch` picks the best stream without re-encoding. It reports the codec, bitrate, and `lossless` in `StreamInfo`.
4. `claims_url` accepts `soundcloud.com` track and set URLs.
5. The source passes the task 30 contract suite.
6. The registry gains one line. No change is needed in the matcher, the pipeline, the store, or the UI (SRS §5.9).

## Open questions for the owner

- The original WAV and premium AAC need an authenticated account. Get the owner's review of the design before any credential handling.
- Check SoundCloud's terms of use at execution time. Record the source and the date.

## Promotion condition

Promote this task when the owner wants SoundCloud material in the library.

## Acceptance criteria

- [ ] The source passes the task 30 contract suite.
- [ ] `StreamInfo` reports the fetched codec and bitrate correctly.
- [ ] The diff outside `source/soundcloud/` is one registry line and the tool declaration.
