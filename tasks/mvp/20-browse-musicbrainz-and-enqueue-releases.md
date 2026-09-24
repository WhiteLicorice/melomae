# 20 — Browse MusicBrainz and enqueue releases

**Status:** TODO
**Phase:** 4 — Desktop UI
**Depends on:** 16, 19
**SRS:** §4, §8
**Stack:** §A
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Let the user find music on MusicBrainz and queue it.

## Scope

1. Search artists, albums, and tracks.
2. An artist page lists release groups by type (album, single, EP, and others).
3. A release picker lists the editions of a release group.
4. A tracklist shows position, title, artist credit, and duration.
5. Tracks and releases already in the library show "In library" (task 16).
6. The user can queue a whole release or single tracks.
7. Use TanStack Query for caching and TanStack Virtual for long lists.

## Out of scope

- The queue screen (task 21)

## Test-first plan

1. Component tests with a mocked bridge cover search results, the discography, and the tracklist.
2. An E2E test runs search → release → queue against the MusicBrainz mock server.

Run the tests. They fail because the screens do not exist.

## Acceptance criteria

- [ ] The E2E journey passes against the built app.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
pnpm test:e2e
pnpm verify
```
