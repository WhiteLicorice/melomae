# B-033 — Correct the chunk budget record anchors

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 05

## Impact

The `file_reserve_bytes` doc comment in `crates/melomae-core/src/inference/chunk_budget.rs:65-67` cites `inference/mod.rs:362`, `:363`, `:411`, and `:467`. After task 33, `output_sum` and `weight_sum` are at `mod.rs:380` and `:381`. The task 05 `Outcome` uses (prediction - measurement) / measurement for the fit rows and (measurement - prediction) / prediction for the acceptance rows without naming the change. The second task 05 review found this (N3).

## Why this does not block MVP

The numbers and the fitted constants are correct. Only the anchors and the sign convention are unclear.

## Expected behavior

The doc comment names the buffers by name or current line, and the record names one error convention.

## Promotion condition

Promote this task with the next change to `chunk_budget.rs`.

## Acceptance criteria

- [ ] The doc comment and the task record agree with the current code and one stated convention.
