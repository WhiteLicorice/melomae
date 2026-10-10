# 17 — Run the job pipeline end to end

**Status:** TODO
**Phase:** 3 — Jobs and pipeline
**Depends on:** 07, 14, 15, 16, 31
**SRS:** §6, §7, §8, §9
**Stack:** §B, §G
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Run each selected file through every stage, from input to mirrored output, with a durable state and a typed event stream.

## Scope

1. Run the stages in this order for each job:
   1. decode (task 03)
   2. restore through `RestoreBackend` (tasks 04 and 05), unless the job has restore turned off
   3. encode (task 06)
   4. copy tags and pictures (task 15)
   5. write provenance and the history row (task 31)
   6. move the output into its mirrored path atomically (SRS §8.1 to §8.3)
2. Run one restore worker at a time.
3. Emit typed events: job ID, stage, progress, ETA, and message.
4. Isolate failures. One failed job does not stop the batch. A failed job can be retried.
5. Remove each job's temporary files when it ends.
6. Never write to an input file.
7. At the end of a batch, build the batch report (SRS §9.5).

## Out of scope

- UI (Phase 4)

## Test-first plan

1. Fake-stage orchestration tests: stage order, a cancel during a stage, failure isolation, and resume after restart.
2. A real run on the task 15 tag fixture inside a nested input folder produces a restored FLAC at the mirrored output path. Every fixture tag, the front cover, and the provenance field read back.
3. A run with restore off writes `model=none` in the provenance field.
4. Every input file is byte-identical after the run.

Run the tests. They fail because the orchestrator does not exist.

## Acceptance criteria

- [ ] The real run produces a tagged, restored FLAC at the mirrored path.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core pipeline
pnpm verify
```
