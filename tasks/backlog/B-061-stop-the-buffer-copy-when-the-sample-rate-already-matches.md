# B-061 — Stop the buffer copy when the sample rate already matches

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 03, 05

## Impact

Most inputs are already at 44.1 kHz. For each one, the decoder copies every channel and then drops the first copy. The peak memory of a decode is twice the audio size.

## Evidence

The ponytail audit of 2026-11-11 read the code at `master` `23f0bcf`. It built and ran nothing. The orchestrator read each cited line again on the same day.

- `crates/melomae-core/src/decode/mod.rs:163-169`: `resample_to` borrows the buffer and returns `self.channels.clone()` when the rate matches.
- The audit cites `decode_file` as the caller. Nobody measured the peak.

## Why this does not block MVP

The chunk chooser of task 05 already reserves memory for the decoded input. The copy costs memory and time, and it does not change the output.

## Expected behavior

`decode_file` moves the buffer into the resample step. A buffer at the target rate comes back with no copy. A caller that must keep its input can still borrow.

## Promotion condition

Promote this task with B-005 or B-024, or when a long file fails the memory limit of SRS §12.3.

## Acceptance criteria

- [ ] A decode of a 44.1 kHz file allocates one audio buffer.
- [ ] Every task 03 fixture decodes to the same samples.
- [ ] The task 05 memory tests pass.
- [ ] A test fails before the change and passes after it.
