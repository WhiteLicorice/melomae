# B-025 — Explain a zero cutoff in triage

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 16

## Impact

The cutoff measurement (`crates/melomae-core/src/triage/cutoff.rs`) returns 0 Hz for a very short clip or an isolated tone, with no diagnostic. The first task 16 review found this (recommendation 3).

## Why this does not block MVP

The verdict is advice, and the user can still restore the file (SRS §4.5).

## Expected behavior

A zero or unreliable cutoff carries a reason, such as too little signal, inside the existing verdict contract.

## Promotion condition

Promote this task with the triage screen (task 24), or when a user reports a confusing verdict.

## Acceptance criteria

- [ ] A test with a short clip and a single tone gets the reason, and fails before the change.
