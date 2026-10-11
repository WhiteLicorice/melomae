# B-029 — Isolate the MP4 decode test files

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 34

## Impact

The `write_temp` test helper (`crates/melomae-core/src/decode/mp4.rs:550`) writes each MP4 test input to a fixed shared folder, `melomae-decode-tests` under the system temp folder, with `fs::write`. It never removes the files. Two test processes that share the temp folder, for example two checkouts that run `cargo test` at once, can overwrite or lock each other's inputs. The task 34 review found this (finding 2). No failure was seen.

## Why this does not block MVP

One test run uses distinct file names per test, so a single `cargo test` has no race.

## Expected behavior

Each test writes its inputs into its own unique directory and removes it at the end.

## Promotion condition

Promote this task when two test runs share one machine in CI, or when a decode test fails from a changed input.

## Acceptance criteria

- [ ] A test with two writers for one logical file name keeps both inputs intact, and fails before the change.
- [ ] No `melomae-decode-tests` files remain after the run.
