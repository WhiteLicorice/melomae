# B-034 — Verify a cached cargo-deny binary again

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 26

## Impact

`tools/cargo_deny.py:85` (`is_complete`) and `:115` accept a cached cargo-deny install when the marker and the binary exist. The SHA-256 is checked only on download. A changed or damaged cached binary then runs the license gate. Reviews 1 to 3 of task 26 found this (F3).

## Why this does not block MVP

The download path verifies the pinned SHA-256. Only a cached file that someone changed later can bypass it.

## Expected behavior

The script checks the cached binary against a recorded hash before each run, and reinstalls or fails on a mismatch. A test changes one byte of the cached binary and expects that result.

## Promotion condition

Promote this task with the next change to `tools/cargo_deny.py`, or before the release build (tasks 27 and 28).

## Acceptance criteria

- [ ] A changed cached binary makes the gate reinstall it or fail, and a test proves it.
