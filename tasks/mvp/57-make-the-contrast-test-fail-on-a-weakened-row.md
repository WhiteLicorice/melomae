# 57 — Make the contrast test fail on a weakened row

**Status:** TODO
**Phase:** 4 — Desktop UI
**Depends on:** 39
**SRS:** §1, §3
**Stack:** §A
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Make `tests/design.test.ts` fail when a row of the `DESIGN.md` contrast table lowers its own minimum or loses a cell.

## Evidence

- The task 39 review (2026-11-18, `C:\mt\reports\39-review.md` findings 1 and 2) changed the body-text row's minimum from 4.5 to 1. The suite stayed green, 5 of 5. Only the 3:1 floor is pinned (`tests/design.test.ts:117`). Text pairs check against their own row (`:138-142`).
- Removing a trailing pipe from a row drops that pair from both themes with no error (`:54-60`). The presence check needs `control-edge` only once (`:121-123`), so the surface row can disappear.

## Scope

1. Require a minimum of 4.5 for every text pair, and of 3 for `control-edge` and `focus`.
2. Fail on a table line that does not split into five columns.
3. Require a `canvas` row and a `surface` row for `control-edge` and for `focus`.

## Test-first plan

1. Each of the three changes above fails on a weakened copy of the table before the fix.

## Acceptance criteria

- [ ] A lowered minimum, a malformed row, and a missing background row each fail the test.
- [ ] The new checks failed first.
- [ ] `pnpm verify` passes.
