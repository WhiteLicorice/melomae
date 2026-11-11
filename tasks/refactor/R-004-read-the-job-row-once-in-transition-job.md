# R-004 — Read the job row once in transition_job

**Status:** TODO
**Severity:** Refactor
**Depends on:** 14

## Impact

Each job transition reads the same row two times. The two reads can also see two versions of the row.

## Evidence

The ponytail audit of 2026-11-11 read the code at `master` `23f0bcf`. It built and ran nothing. The orchestrator read each cited line again on the same day.

- `crates/melomae-core/src/store/mod.rs:436-437`: `self.job(id)?.state`, then `self.job(id)?.restore_enabled`.
- The audit reports that `restore_enabled` never changes after the insert, and that the conditional update protects the state. The orchestrator did not check either point.

## Why this is a refactor

The result is correct today. The second read is wasted work.

## Expected behavior

`transition_job` reads the job once and uses both fields from that value.

## Promotion condition

Promote this task with the next change to `transition_job`.

## Smallest correct fix boundary

- Change the two lines only.

## Test-first plan

- The gate is the characterization test. The transition matrix and the race tests of task 14 must pass with no change.

## Acceptance criteria

- [ ] `transition_job` calls `self.job` once.
- [ ] `cargo test -p melomae-core store` gives the same counts before and after the change.
