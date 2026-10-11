# B-024 — Bound triage memory on long files

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 16

## Impact

`triage::triage_file` (`crates/melomae-core/src/triage/mod.rs:116`) calls `decode_file` and holds the whole decoded file before it samples bounded FFT windows. A multi-hour recording can use several GB during a scan. The first task 16 review found this (recommendation 1). It is a sibling of B-005, which covers the restore engine.

## Why this does not block MVP

Typical tracks are under 10 minutes, and the cutoff samples at most 128 positions.

## Expected behavior

Triage decodes only the frames that the selected analysis windows need, or decodes as a stream. The verdicts stay the same.

## Promotion condition

Promote this task with B-005, or when a user scans multi-hour recordings.

## Acceptance criteria

- [ ] A test measures the triage peak for a long input and fails before the change.
- [ ] The calibration verdicts do not change.
