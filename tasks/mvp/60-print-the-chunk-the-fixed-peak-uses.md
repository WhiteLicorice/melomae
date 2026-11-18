# 60 — Print the chunk the fixed peak uses

**Status:** TODO
**Phase:** 1 — Restoration engine
**Depends on:** 46
**SRS:** §6, §12
**Stack:** §C
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Make the `chunk budget:` line for a fractional `--chunk-seconds` name the chunk that its predicted peak describes.

## Evidence

- The task 46 review (non-blocking 4): `ChunkChoice::describe` (`crates/melomae-cli/src/restore.rs:229-232`) computes the peak with `seconds.round()` and prints the unrounded `{seconds}`.
- At 6.5 s the line printed `chunk 6.5s: ... predicted inference peak 5079987029 bytes`. That value is `predicted_peak_bytes(7)`. The engine runs 286650 samples (`crates/melomae-core/src/inference/mod.rs:250`).

## Scope

1. Print the whole second used for the peak, or label the peak as the rounded chunk's.

## Test-first plan

1. A unit test of `describe` at 6.5 s fails before the fix.

## Acceptance criteria

- [ ] The line for a fractional chunk names the chunk its peak describes.
- [ ] The new test failed first.
- [ ] `cargo test -p melomae-cli` passes.
