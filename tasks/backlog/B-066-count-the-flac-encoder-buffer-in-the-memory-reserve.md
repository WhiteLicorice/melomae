# B-066 — Count the FLAC encoder buffer in the memory reserve

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 05, 06

## Impact

The memory reserve of the chunk chooser does not count one full-size buffer of the FLAC encoder. On a long file, the peak at encode time can exceed the budget of SRS §12.3.

## Evidence

The ponytail audit of 2026-11-11 read the code at `master` `23f0bcf`. It built and ran nothing. The orchestrator read each cited line again on the same day.

- `crates/melomae-core/src/inference/chunk_budget.rs:80-82`: the reserve is `(2 * channels + 2) * frames` samples.
- `crates/melomae-core/src/encode/flac.rs:33`: the encoder allocates `frames * channels` interleaved samples.
- The audit reports that the WAV writer works in blocks of 4,096 frames. The orchestrator did not read it.
- Inferred, not measured: the missing buffer moves the peak past the budget on a long file.

## Why this does not block MVP

A normal track is short. The decoded input, not this buffer, sets the peak for most files.

## Expected behavior

The FLAC encoder writes its samples in blocks, as the WAV writer does. As an alternative, the reserve counts the interleaved buffer.

## Promotion condition

Promote this task with B-005, or when a long file exceeds the memory limit.

## Acceptance criteria

- [ ] A measurement records the encode peak for a 60-minute input before and after the change.
- [ ] The FLAC output stays bit-identical for every task 06 fixture.
- [ ] A test fails before the change and passes after it.
