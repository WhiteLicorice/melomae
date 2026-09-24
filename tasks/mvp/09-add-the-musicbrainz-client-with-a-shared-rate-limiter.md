# 09 — Add the MusicBrainz client with a shared rate limiter

**Status:** TODO
**Phase:** 2 — Metadata and acquisition
**Depends on:** 00
**SRS:** §4, §8
**Stack:** §F
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Give `melomae-core` a MusicBrainz WS/2 client that never exceeds the service's rate limit.

## Evidence

- MusicBrainz allows 1 request per second per IP (checked on 2026-09-24 at `musicbrainz.org/doc/MusicBrainz_API/Rate_Limiting`). It declines requests above the limit.
- MusicBrainz requires a User-Agent with contact information: `Application name/<version> ( contact-url )`.
- Bootstrap item 1 supplies the contact. Until then, a development placeholder is allowed.

## Scope

1. Use the WS/2 JSON API for:
   - search: artist, release-group, release, and recording
   - browse: an artist's release groups, paged at 100
   - lookup: a release with recordings, ISRCs, artist credits, media, and genres
2. Run one process-wide limiter at 1 request per second. Every caller shares it.
3. On HTTP 503, back off and retry.
4. Send `Melomae/<version> ( <contact> )`. The contact comes from build configuration.
5. Keep an in-memory LRU cache of responses.
6. Map the responses to typed Rust structs. Keep the MBIDs, durations, positions, and ISRCs.

## Out of scope

- The browse UI (task 20)
- A disk cache

## Test-first plan

1. Serve recorded JSON through a mock HTTP server (for example `wiremock`). Assert the typed results.
2. Assert that a burst of 5 calls takes at least 4 s.
3. Assert that a 503 is retried.
4. Add one live smoke test that runs only when `MELOMAE_LIVE_TESTS=1`.

Run the tests. They fail because the client does not exist.

## Acceptance criteria

- [ ] The limiter holds under concurrent callers.
- [ ] Release builds fail to compile, or fail at start, when the contact is the placeholder. Task 29 depends on this check.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core musicbrainz
pnpm verify
```
