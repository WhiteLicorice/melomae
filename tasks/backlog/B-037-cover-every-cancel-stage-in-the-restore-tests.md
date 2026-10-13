# B-037 — Cover every cancel stage in the restore tests

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 07

## Impact

The core cancel tests at `crates/melomae-core/tests/encode_formats.rs:416` and `:446` set the flag before the call, so they return before the temporary file exists. They cannot detect a broken cleanup during a WAV or Opus encode. The early CLI signal test at `crates/melomae-cli/tests/restore_signals.rs:317` waits for the start line, which the CLI prints after decode, so no test cancels during decode. Review 2 of task 07 found this (N1).

## Why this does not block MVP

The reviewer's own core probe cancelled after the temporary file existed for FLAC, WAV, and Opus, and each left no file. All signal tests passed in five more runs.

## Expected behavior

Core tests cancel after the temporary file exists for WAV and Opus. A controlled test cancels during decode. The test names claim only the stage that they observe.

## Promotion condition

Promote this task with the next change to the encoder transaction or to the CLI cancel path.

## Acceptance criteria

- [ ] The new tests fail when the cleanup on a WAV or Opus cancel is removed, and pass with it.
