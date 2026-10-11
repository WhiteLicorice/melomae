# B-022 — Test the encoder error-cleanup paths

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 06

## Impact

`encode` in `crates/melomae-core/src/encode/mod.rs` removes its temporary file when the encoder fails and when the rename fails. No committed test covers those two branches. The second task 06 review exercised both with its own probes on Windows, and both worked. A later change can break the cleanup with no failing test. The input check also calls `frames()` before it checks the channel count, and `frames()` reads the first channel. `AudioBuffer::new` rejects zero channels today, so no caller can reach that order.

## Why this does not block MVP

Both branches work today, and the public constructor prevents a zero-channel buffer.

## Expected behavior

Committed tests prove that a failed encode and a failed rename each keep an older target unchanged and leave no temporary file.

## Promotion condition

Promote this task with the next change to `encode/mod.rs`, or when another constructor can make a zero-channel buffer.

## Acceptance criteria

- [ ] A public test encodes three-channel audio as Opus over an older target, and checks that the target is unchanged and no `.tmp` file remains.
- [ ] A public test renames onto an existing directory, with the same checks.
- [ ] Each test fails when its cleanup branch is removed.
- [ ] The input check tests the channel count before it calls `frames()`.
