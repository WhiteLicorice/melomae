# 29 — Run the release-candidate journey

**Status:** TODO
**Phase:** 5 — Packaging and release
**Depends on:** 21, 23, 24, 27, 28, 32
**SRS:** §1, §2, §3, §4, §5, §6, §7, §8, §9, §10, §11, §12
**Stack:** §J, §K
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Prove the whole product through the path a real user takes, and record the numbers against the targets.

## Scope

On a fresh Windows user profile, run these steps in order:

1. Install from the task 27 installer.
2. Complete the first-run flow with the network turned off.
3. Add a nested folder that holds the task 16 calibration fixtures and the task 15 tag fixture.
4. Check each verdict and the default selection.
5. Restore one file on CPU and one on GPU.
6. Check the tags, the cover, and the provenance field in a third-party player.
7. Check that every input file is unchanged.
8. Run the volume-matched compare and ten blind ABX trials.
9. Uninstall.

Record the time for each step and compare it with SRS §12 and Stack §K. Repeat steps 2, 5, and 8 under WSLg.

## Blocking inputs

This task cannot finish before bootstrap item 6 (the name check) is complete. If it is missing, follow the human-input protocol.

## Out of scope

- A public release. The owner decides that after this task.

## Test-first plan

This task runs the existing suites and a manual journey. It adds no new test. Record the manual checks in `Outcome`.

## Acceptance criteria

- [ ] Every step passes.
- [ ] Every SRS §12 target has a measured value in `Outcome`.
- [ ] `Outcome` lists what stayed unverified and why.
- [ ] `pnpm verify` passes.

## Verify

```powershell
pnpm verify
```

Record the journey in `Outcome`.
