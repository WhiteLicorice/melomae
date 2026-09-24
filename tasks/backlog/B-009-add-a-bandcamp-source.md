# B-009 — Add a Bandcamp source

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 30, 17

## Impact

Many independent artists publish on Bandcamp and not on YouTube Music. Some Bandcamp tracks are also available as lossless downloads.

## Evidence

Checked on 2026-09-24:

- yt-dlp lists the extractors `Bandcamp`, `Bandcamp:album`, `Bandcamp:user`, and `Bandcamp:weekly` (`supportedsites.md`). The managed yt-dlp binary from task 11 can serve this source.
- The public Bandcamp stream is `mp3-128` (`yt_dlp/extractor/bandcamp.py:460`).
- A track with a free-download page offers other encodings (`bandcamp.py:223-280`). Those encodings can be lossless.
- Purchased downloads need the user's Bandcamp account.

## Why this does not block MVP

The owner put real source wiring in the backlog on 2026-09-24. The MVP proves the interface with YouTube Music and the task 30 test sources.

## Expected behavior

1. `BandcampSource` implements `StreamSource` in `source/bandcamp/`. It declares yt-dlp in `required_tools()`.
2. It searches Bandcamp and maps results to `Candidate`. A track on the artist's own page maps to `official_audio = true`.
3. `fetch` uses the best free encoding. It reports `lossless = true` for a lossless encoding. SRS §6.10 then skips restore.
4. `claims_url` accepts `*.bandcamp.com` track and album URLs.
5. The source passes the task 30 contract suite.
6. The registry gains one line. No change is needed in the matcher, the pipeline, the store, or the UI (SRS §5.9).

## Open questions for the owner

- Handling purchased downloads needs the user's account credentials or cookies. Get the owner's review of the design before any credential handling.
- Check Bandcamp's terms of use at execution time. Record the source and the date.

## Promotion condition

Promote this task when the owner wants a second source, or when task 12 data shows many tracks with no YouTube Music candidate.

## Acceptance criteria

- [ ] The source passes the task 30 contract suite.
- [ ] A lossless free download skips restore.
- [ ] The diff outside `source/bandcamp/` is one registry line and the tool declaration.
