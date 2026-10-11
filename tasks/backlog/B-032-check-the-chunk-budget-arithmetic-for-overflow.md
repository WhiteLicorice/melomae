# B-032 — Check the chunk budget arithmetic for overflow

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 05

## Impact

`predicted_peak_bytes` and the reserve API in `crates/melomae-core/src/inference/chunk_budget.rs` (near lines 80, 132, and 143) multiply and add `u64` values without checks. A deliberately huge channel count, frame count, or reserve overflows. Both task 05 reviews found this (N2).

## Why this does not block MVP

A decoded file on a real machine cannot reach these values.

## Expected behavior

The arithmetic uses checked or saturating operations, or the API bounds its inputs, and an overflow returns the memory error.

## Promotion condition

Promote this task with the job queue (task 17), or when the chooser takes untrusted shapes.

## Acceptance criteria

- [ ] A test with `u64::MAX` channels and a `u64::MAX` reserve gets an error, not a panic or a wrong chunk, and fails before the change.
