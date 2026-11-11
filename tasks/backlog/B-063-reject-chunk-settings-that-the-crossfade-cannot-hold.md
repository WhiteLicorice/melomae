# B-063 — Reject chunk settings that the crossfade cannot hold

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 04

## Impact

Two chunk settings fail in a bad way. An overlap that is not smaller than the chunk makes `chunk_starts` loop until memory runs out. An overlap of one sample makes the restore fail with `UncoveredOutput`.

## Evidence

The ponytail audit of 2026-11-11 read the code at `master` `23f0bcf`. It built and ran nothing. The orchestrator read each cited line again on the same day.

- `crates/melomae-core/src/inference/mod.rs:272-279`: `chunk_starts` is public. Its hop is `chunk.saturating_sub(overlap)`. With a hop of zero, the loop adds the same start each time.
- `crates/melomae-core/src/inference/mod.rs:304-310`: `linspace(1)` returns `[0.0]`. The fade-in and the fade-out of a one-sample overlap are then both zero, so the weights sum to zero.
- `crates/melomae-core/src/inference/mod.rs:436`: a zero weight sum returns `UncoveredOutput`.
- The audit reports that the internal path validates the settings first. The orchestrator did not read that validation. Nobody ran either setting.

## Why this does not block MVP

The default settings and the automatic chunk size of task 05 give an overlap of 44,100 samples. No user path sets either bad value.

## Expected behavior

`chunk_starts` returns an error when the overlap is not smaller than the chunk. `resolve_chunking` rejects an overlap below two samples with a typed settings error.

## Promotion condition

Promote this task when a setting or an API lets a caller choose the overlap.

## Acceptance criteria

- [ ] `chunk_starts(10_000, 1_000, 1_000)` returns an error and does not loop.
- [ ] A restore with a one-sample overlap returns a typed settings error.
- [ ] The existing chunk tests pass with no change.
- [ ] A test fails before the change and passes after it.
