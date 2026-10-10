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

## Out of scope

- Any online lookup (SRS §5.6)

## Test-first plan

1. Component tests: the default selection for each verdict, select-all by verdict, and the estimate notice.
2. Planner tests cover mirroring, skip-existing, and the files `Florida!!!.opus` and `Habits (Stay High).opus`.
3. An E2E test drops a folder, sees the verdicts, starts the batch, and sees the jobs in the queue.

Run the tests. They fail because the screen does not exist.

## Acceptance criteria

- [ ] A batch of three fixtures produces three outputs at their mirrored paths, with their tags.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
pnpm test:e2e
pnpm verify
```
