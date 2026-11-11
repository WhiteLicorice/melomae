# B-062 — Stop a second app instance from requeueing live jobs

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 14, 17

## Impact

A user can start the app twice. The second copy calls the recovery step and moves the running job of the first copy to `queued`. Both copies can then restore the same file.

## Evidence

The ponytail audit of 2026-11-11 read the code at `master` `23f0bcf`. It built and ran nothing. The orchestrator read each cited line again on the same day.

- `crates/melomae-core/src/store/mod.rs:358-364`: `recover_interrupted_jobs` moves every `restoring`, `encoding`, and `tagging` job to `queued`. Its comment says that the app calls it once at startup. It has no owner check.
- `src-tauri/src/lib.rs:7-14`: the app has no single-instance guard.
- The failure sequence is inferred. No worker exists yet, so nobody reproduced it.

## Why this does not block MVP

No worker runs in the app until task 17 is done.

## Expected behavior

The app holds a single-instance guard. A second start focuses the first window and exits. As an alternative, a job holds an owner lock and the recovery step moves only a job with no live owner. The guard is the smaller change.

## Promotion condition

Promote this task when task 17 is DONE and before the first release.

## Acceptance criteria

- [ ] A second start of the app does not change the state of a running job.
- [ ] One app instance still recovers its jobs after a crash.
- [ ] A test fails before the change and passes after it.
