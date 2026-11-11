# B-057 — Export a stateful streaming model

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** B-056

## Impact

The restore interface takes one whole file. A live mode must restore a short block, keep the state, and continue with the next block. Without a stateful model, each block must run again over its full receptive field, and the compute cost rises by several times.

## Evidence

- Confirmed: `RestoreBackend::restore` takes the channels of one file (`crates/melomae-core/src/inference/mod.rs:208-217`). No block-level interface exists.
- Inferred, not confirmed: an export that caches the recent frames of each time convolution, and the STFT overlap, can reproduce one whole-file run at about the same compute cost. The reference is a whole-file run. It is not the 20 s chunked output of today (`docs/melomae-stack.md:123`).
- B-056 gives the measured receptive field. This record is wrong if that field is not bounded.
- Unverified: the Apollo paper says only that the model "supports streaming processing" with causal convolution and a causal Roformer. It reports no streaming delay and no quality for that mode.

## Why this does not block MVP

The MVP restores whole files through the chunked path.

## Expected behavior

1. Reuse the loaded weights, as `tools/export/real_dsp.py` does for the STFT. Do not retrain.
2. Add a cache input and a cache output for each time convolution and for the STFT overlap.
3. Add a block-level, stateful interface beside `RestoreBackend`. Do not change the file-level trait.
4. Compare the block-by-block output with one whole-file run of the same input.

This work is close to B-005, B-024, task 05, and task 36. Do not merge any of them into this record without the owner's decision.

## Promotion condition

Promote this record when B-056 records a bounded receptive field that the owner accepts as a delay.

## Acceptance criteria

- [ ] For block sizes of 0.1, 0.25, 0.5, and 1 s, the block-by-block output matches one whole-file run within the tolerance that B-053 set for the live mode.
- [ ] The tests fail first against an export with no cache.
- [ ] A record holds the compute cost for each second of audio, against the whole-file run.
- [ ] `pnpm verify` passes.
