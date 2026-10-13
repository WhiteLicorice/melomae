# 36 — Fix the restore output when a chunk is shorter than the input

**Status:** TODO
**Phase:** 1 — Restoration engine
**Depends on:** 04, 05, 07
**SRS:** §6, §12
**Stack:** §C
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Make the restored audio independent of the chunk length that the chooser picks, within the §12.1 tolerances.

## Evidence

- The CLI test `e2e_restore_wav_matches_golden` (`crates/melomae-cli/tests/restore_e2e.rs:276`, assertion at `:297`) restores an 18 s input built from three copies of `tools/parity/input_wav.wav` and compares it with the golden.
- On 2026-10-14 the task 15 reviewer ran it at `master` `9142778` and at `task/15` `f0adbe5`. Both runs gave the same result. The chooser picked a 17 s chunk from live free memory, the restore ran 2 chunks, and channel 0 gave max abs 6.710718e-1 and RMSE 1.951308e-2 against the limits 1e-3 and 1e-4. Evidence: `rev15f-cli-golden-master.txt` and `rev15f-cli-golden-alone.txt` in the orchestrator scratch folder of that session.
- The task 07 gate passed the same test, because more free memory gave a chunk of at least the input length, so one chunk covered the file. The test result therefore depends on free memory, and the gate hides the defect.
- The core golden contract uses 6 s chunks (`crates/melomae-core/tests/inference_parity.rs:44`) and passes.
- Inferred, not confirmed: the defect is in the chunk layout when the chunk is shorter than the input but close to its length, for example the padded-start clamp at the file end (Stack §C, the port of `C:\Lab\Apollo\inference.py:86-221`). A run with a fixed 17 s chunk on the 18 s input, against a reference made with the upstream Python code at the same settings, confirms or rejects this.

## Scope

1. Reproduce the error with a fixed chunk length, not with free memory.
2. Find the cause in the chunk layout or the overlap-add, and fix it.
3. Make `e2e_restore_wav_matches_golden` independent of free memory: run it at fixed chunk settings, and add the settings that failed.
4. Add a core test matrix over chunk lengths around the input length (for example 6, 16, 17, 18, and 19 s on the 18 s input) against the reference.

## Out of scope

- A new golden at other model settings (B-039 covers the reference matrix in the CLI).

## Test-first plan

1. A core test restores the 18 s input with a fixed 17 s chunk and compares it with the golden. It fails now with an error near 0.67.

## Acceptance criteria

- [ ] Every chunk length in the matrix matches the reference within the §12.1 tolerances.
- [ ] The CLI golden test no longer depends on free memory.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.
