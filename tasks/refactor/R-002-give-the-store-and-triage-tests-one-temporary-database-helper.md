# R-002 — Give the store and triage tests one temporary-database helper

**Status:** TODO
**Severity:** Refactor
**Depends on:** 34

## Impact

Seven files define the same `TempDb` test helper. A change to its name scheme or its cleanup must go into each copy. B-030 already records that the scheme is weak.

## Evidence

The ponytail audit of 2026-11-11 read the code at `master` `23f0bcf`. It built and ran nothing. The orchestrator read each cited line again on the same day.

- A search for `struct TempDb` on 2026-11-11 found seven definitions: `crates/melomae-core/src/store/mod.rs:654`, `src/store/race_tests.rs:26`, `tests/store_e2e.rs:12`, `tests/store_recovery.rs:13`, `tests/store_regressions.rs:14`, `tests/triage_round1_fixes.rs:22`, and `tests/triage_scan.rs:21`.

## Why this is a refactor

Every copy works. The change removes duplicate test code and changes no product behavior.

## Expected behavior

One helper lives in `tests/setup/mod.rs`. One more serves the unit tests in `src/store`, because an integration test cannot see a `#[cfg(test)]` item. No other file defines `TempDb`.

## Promotion condition

Promote this task with B-030, or before the next change to the helper.

## Smallest correct fix boundary

- Move the helper. Do not change its name scheme in the same change. B-030 owns that.

## Test-first plan

- The gate is the characterization test. Record the `cargo test --workspace` counts before the change.

## Acceptance criteria

- [ ] A search for `struct TempDb` finds two definitions at most.
- [ ] `cargo test --workspace` gives the same counts before and after the change.
