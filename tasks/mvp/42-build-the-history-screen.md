# 42 — Build the history screen

**Status:** TODO
**Phase:** 4 — Desktop UI
**Depends on:** 19, 23, 31
**SRS:** §8, §10
**Stack:** §A
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Let the user find a past restore, read what Melomae did to the file, and open the compare player from it.

## Evidence

- SRS §8.8 keeps a history row for each restore (`docs/melomae-srs.md:89`).
- Task 31 writes the row. Its out-of-scope list names the history screen and points at tasks 19 and 23.
- Task 19 adds the History entry to the sidebar. Task 18 exposes a history command.
- A search of `tasks/mvp` on 2026-11-11 found no record that builds the screen.

## Scope

1. A table of the history rows, newest first: date, file name, verdict, output format, device, and result.
2. Virtualize the table with TanStack Virtual (Stack §A). Read the rows in pages, newest first. `Store::history` reads every row in insertion order with no limit (`crates/melomae-core/src/store/mod.rs:551-559`). Add a page query with a limit and a cursor, and keep `history` for the tests.
3. A text field filters the rows by file name.
4. A row opens its details: the input path, the output path, the input SHA-256, the settings, and the provenance value. The provenance value has a copy action.
5. A row with a finished restore has a Compare action. It opens the task 23 player. The action is disabled, with the reason, when SRS §10.8 makes the compare unavailable.
6. Show an empty state with a link to the Restore screen. Show a loading state and an error state with a retry.
7. Use the verdict labels from task 24 when that task is done. Do not show an internal verdict name.

`tasks/ux/interface-proposal.md` section 3.5 holds the layout. Its MVP ruling removes the other filters.

## Out of scope

- Filters by date, result, verdict, or format
- Deleting or exporting history rows

## Test-first plan

1. Component tests cover the empty state, the loading state, and the error state.
2. A component test with a 5,000-row fixture renders only the visible rows.
3. A component test filters by a file name and sees only the matching rows.
4. A component test opens a row and copies the provenance value.
5. A component test with a missing input file finds the Compare action disabled and reads the reason.
6. An axe scan of the screen passes (task 40), when that task is done.
7. A store test reads two pages and finds that they equal the newest rows of the full list, in order.

Run the tests. They fail because the screen does not exist.

## Acceptance criteria

- [ ] After one restore in the built app, the History screen shows its row.
- [ ] The Compare action opens the player for that row.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
pnpm test
pnpm test:e2e
pnpm verify
```
