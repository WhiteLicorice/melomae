# B-030 — Harden test temporary paths

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 34

## Impact

`TempDir::new` (`crates/melomae-core/tests/setup/mod.rs:56`) and the `TempDb::new` helpers (for example `crates/melomae-core/src/store/race_tests.rs:31`) name paths from the process id, the clock, and a counter that restarts in each process. `TempDb` does not reserve its path, and `TempDir` assumes the temp parent folder exists. A reused process id with a repeated clock value and a stale file, or a custom temp folder that is missing, can still fail. The task 34 review found this (finding 3).

## Why this does not block MVP

Ordinary parallel tests on one machine get distinct names, and both hosted CI runners pass.

## Expected behavior

Each helper reserves its path with exclusive creation and retries on a collision, and creates a missing parent folder.

## Promotion condition

Promote this task if a test fails from a stale temporary path, or when tests run in a custom temp folder.

## Acceptance criteria

- [ ] A test with an existing candidate path and a missing parent folder gets a fresh path, and fails before the change.
