# 22 — Review uncertain matches

**Status:** TODO
**Phase:** 4 — Desktop UI
**Depends on:** 12, 21
**SRS:** §5
**Stack:** §A
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Let the user decide each `uncertain` or `none` match.

## Scope

1. Show the MusicBrainz track next to its ranked candidates: title, channel, duration difference, and thumbnail.
2. The user can accept a candidate, choose another, skip the track, open a candidate in the browser, or paste a URL.
3. **Source interface (core requirement).** Show the source name on each candidate. A pasted URL goes through `SourceRegistry` routing (task 30). An unclaimed URL shows "No source supports this link."
4. A pasted URL goes through the same duration check. A failed check shows a warning, but the user can still accept.
5. Every decision persists in the store (task 14) and moves the job forward.

## Out of scope

- In-app preview playback of YouTube candidates

## Test-first plan

1. Component tests cover each decision, and an unclaimed pasted URL.
2. An E2E test resolves one uncertain job and sees it move to downloading.

Run the tests. They fail because the screen does not exist.

## Acceptance criteria

- [ ] Every decision survives an app restart.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
pnpm test:e2e
pnpm verify
```
