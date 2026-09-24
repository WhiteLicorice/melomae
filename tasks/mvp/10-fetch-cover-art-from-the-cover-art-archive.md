# 10 — Fetch cover art from the Cover Art Archive

**Status:** TODO
**Phase:** 2 — Metadata and acquisition
**Depends on:** 09
**SRS:** §8
**Stack:** §F
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Fetch the front cover for a release so task 15 can embed it.

## Scope

1. Fetch the release front cover at the 1200 px size.
2. If the release has no front cover, fetch the release-group front cover.
3. A missing cover is not an error. Return "no cover".
4. Confirm the Cover Art Archive's current rate policy and redirect behavior live at execution time. Record the source and date in `Outcome`.

## Out of scope

- Embedding (task 15)

## Test-first plan

Use a mock HTTP server for three cases: a release hit, the release-group fallback, and a 404 with no cover. Run the tests. They fail because the client does not exist.

## Acceptance criteria

- [ ] All three cases behave as described.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core coverart
pnpm verify
```
