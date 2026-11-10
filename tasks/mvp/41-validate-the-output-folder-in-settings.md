# 41 — Validate the output folder in Settings

**Status:** TODO
**Phase:** 4 — Desktop UI
**Depends on:** 19
**SRS:** §3, §8
**Stack:** §A
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Tell the user that the output folder is unusable when they choose it, and not when the first batch fails.

## Evidence

- Task 19 scope item 4 lists the default output folder as a setting. Its tests cover a successful save only.
- SRS §3.3 requires a default output folder (`docs/melomae-srs.md:30`).
- No record checks that the folder exists or accepts a write.
- The impeccable audit of 2026-11-11 at `master` `0220b19` named this gap.

## Scope

1. Check the folder when the user picks it and when the settings save. Write a temporary file in the folder and delete it.
2. On failure, show an inline error and keep the last good value.
3. Show the save state of each field: saving, saved, or failed with a retry.
4. Check the folder again at launch. A missing folder shows the error on the Restore screen and in Settings.
5. Use the same check in the first-run flow of task 25.

## Out of scope

- A free-space check

## Test-first plan

1. A core or bridge test gives a read-only folder and expects a typed error.
2. A component test shows the inline error and the unchanged stored value.
3. A component test covers the saved state and the failed state.

Run the tests. They fail because the check does not exist.

## Acceptance criteria

- [ ] A read-only folder shows an inline error and is not saved.
- [ ] A folder that was deleted after the save shows an error at the next launch.
- [ ] The check leaves no file in the folder.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
pnpm test
pnpm verify
```
