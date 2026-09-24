# 24 — Restore local files and folders

**Status:** TODO
**Phase:** 4 — Desktop UI
**Depends on:** 07, 21
**SRS:** §9
**Stack:** §A, §D, §F
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Restore audio files the user already owns, in batch. This replaces the owner's `run_batch.py` workflow.

## Evidence

`C:\Lab\Apollo\run_batch.py` recurses `input\`, mirrors the tree into `output\`, skips outputs that exist, and continues after a failure. `C:\Lab\Apollo\metadata_utils.py` copies tags to the output.

## Scope

1. The user drops or picks files or folders.
2. Folders are searched recursively for mp3, m4a, wav, aiff, aif, flac, opus, ogg, and webm files.
3. The output mirrors the input tree inside the chosen output folder.
4. An output that already exists is skipped.
5. A failed file does not stop the batch.
6. Tags from the source file are copied to the output with lofty.
7. The jobs appear in the queue (task 21).

## Out of scope

- MusicBrainz lookup for local files

## Test-first plan

1. Planner tests cover mirroring, skip-existing, and the files `Florida!!!.opus` and `Habits (Stay High).opus`.
2. An E2E test drops a folder and sees the jobs in the queue.

Run the tests. They fail because the planner does not exist.

## Acceptance criteria

- [ ] A batch of three fixtures produces three tagged outputs.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
pnpm test:e2e
pnpm verify
```
