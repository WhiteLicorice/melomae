# B-038 — Cover awkward names, multichannel input, and Opus output in the CLI tests

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 07

## Impact

The CLI fixture loop in `crates/melomae-cli/tests/restore_e2e.rs` restores the task 03 inputs to FLAC only. It has no file name with spaces, non-ASCII characters, or a long path, no input with more than two channels, and no independent decode of real CLI Opus output. Review 1 of task 07 found this gap.

## Why this does not block MVP

The core decode and encode tests cover these inputs and formats, and the CLI holds no pipeline logic of its own.

## Expected behavior

CLI tests restore an awkwardly named file, a multichannel input (or check its documented error for Opus), and an Opus output that an independent decoder reads.

## Promotion condition

Promote this task before task 17 uses the CLI path as its reference, or when a user reports a name or channel failure.

## Acceptance criteria

- [ ] Each case runs through the real binary and passes.
