# 24 — Add folders and review the triage

**Status:** TODO
**Phase:** 4 — Desktop UI
**Depends on:** 07, 16, 21
**SRS:** §4, §8, §9
**Stack:** §A, §D, §F
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Give the user the main screen: add files or folders, review the triage verdicts, choose files, and start the batch. This replaces the owner's `run_batch.py` workflow.

## Evidence

`C:\Lab\Apollo\run_batch.py` recurses `input\`, mirrors the tree into `output\`, skips outputs that exist, and continues after a failure. `C:\Lab\Apollo\metadata_utils.py` copies tags to the output.

## Scope

1. The user drops or picks files or folders, and picks the output folder. The default comes from settings.
2. The screen runs triage (task 16) and lists each file with its verdict, codec, bitrate, and cutoff.
3. The default selection follows SRS §4.5. The user can select or clear any file, or every file with one verdict.
4. The screen says that verdicts are estimates (SRS §4.4).
5. Start sends the selected files to the queue (task 21).
6. After the batch, the screen shows the batch report (SRS §9.5).
7. Show these states: an empty screen with the two add actions, a running scan with a file count and a cancel, and an unreadable file with its reason on the row (SRS §4.3). The batch report has a row for each failed file (SRS §9.2).
8. Show a plain label and a one-line explanation for each verdict. Do not show the internal names, such as `suspect-lossless`. Use the five labels, shapes, and explanations from section 1 of the proposal. Show the cutoff in kHz, and as a marker on a 0 to 22.05 kHz track. The marker comes from the stored measurement. A verdict has a text label and does not use color alone.
9. Virtualize the list with TanStack Virtual (Stack §A). Space toggles the focused row, Shift selects a range, and the selected count shows next to Start. A long file name shows in full on focus and on hover.
10. Settle the drop path before the screen is built. Tauri 2 may intercept an OS file drop, so an HTML5 `drop` handler may never fire. This is inferred and not confirmed. Read the current Tauri documentation, pick the Tauri drag-drop event or the HTML5 path, and record the pick. The picker stays reachable from the keyboard.
11. The batch report has a cancelled count, next to the restored, skipped, and failed counts. Amend SRS §9.5 (`docs/melomae-srs.md:97`) in the same change so that it names the cancelled files.
12. Build one table. Do not build the Sources pane, the drawer, or the sort (backlog B-052).

`tasks/ux/interface-proposal.md` section 3.2 proposes the layout, the labels, and the cutoff meter. Read its MVP ruling first. The ruling wins over the section.

## Out of scope

- Any online lookup (SRS §5.6)

## Test-first plan

1. Component tests: the default selection for each verdict, select-all by verdict, and the estimate notice.
2. Planner tests cover mirroring, skip-existing, and the files `Florida!!!.opus` and `Habits (Stay High).opus`.
3. An E2E test drops a folder, sees the verdicts, starts the batch, and sees the jobs in the queue. The drop goes through the path that scope item 10 picked, not through a simulated DOM event.
4. Component tests cover the empty state, the running scan with its cancel, and a corrupt fixture with its reason.
5. A copy test checks the five visible labels and finds no internal verdict name.
6. A component test with a 5,000-row fixture renders only the visible rows, and covers Space and the range selection.

Run the tests. They fail because the screen does not exist.

## Acceptance criteria

- [ ] A batch of three fixtures produces three outputs at their mirrored paths, with their tags.
- [ ] A batch with one failed file still shows the other outputs and the failed row.
- [ ] A batch with one cancelled file reports it as cancelled, and SRS §9.5 names that result.
- [ ] `Outcome` records the drop path and the source that confirmed it.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
pnpm test:e2e
pnpm verify
```
