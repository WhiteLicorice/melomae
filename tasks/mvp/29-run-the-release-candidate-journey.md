# 29 — Run the release-candidate journey

**Status:** TODO
**Phase:** 5 — Packaging and release
**Depends on:** 20, 21, 22, 23, 24, 27, 28
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
2. Complete the first-run flow.
3. Search for an album and queue it.
4. Resolve one review.
5. Let the downloads finish.
6. Restore one track on CPU and one on GPU.
7. Check the tags and the cover in a third-party player.
8. Run the A/B compare.
9. Restore a local folder.
10. Uninstall.

Record the time for each step and compare it with SRS §12 and Stack §K. Repeat steps 2, 6, and 8 under WSLg.

## Blocking inputs

This task cannot finish before bootstrap items 1 (the MusicBrainz contact), 5 (the YouTube risk acceptance), and 6 (the name check) are complete. If one is missing, follow the human-input protocol.

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
