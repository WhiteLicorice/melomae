# B-056 — Measure the time receptive field of the stock model

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 01

## Impact

A live mode must wait for the future audio that the model reads. That lookahead is the floor of the start delay. If the released model reads about half a second ahead, it can stream with no retraining. If it reads much more, the live mode needs new weights.

## Evidence

Confirmed by reading `tools/export/vendor/look2hear/models/apollo.py` on 2026-11-11:

- `:189` builds the band Roformer with `causal=False`. `:190` builds the time block `ICB` with its default, and `:171` sets that default to `causal=False`. The causal path at `:156-166` exists, and nothing turns it on.
- The Roformer attends across the bands of one frame (`:197` reshapes to `B*T, -1, nband`). It does not read other frames.
- Each time convolution is depthwise with kernel 7 and no dilation (`:142`, `:149`). `ICB` holds 3 of them (`:174-176`). The model has 6 layers (`docs/melomae-stack.md:34`).
- `RMSNorm` averages over channels in one frame (`:21`). The band normalization is per frame (`:261-265`). The class `RMVN` (`:25-46`) is defined and not used.
- The STFT window is 882 samples with a hop of 441 at 44.1 kHz (`docs/melomae-stack.md:34`).

Inferred from those lines, not measured: 18 convolutions reach 3 frames each way, so the field is 54 frames each way. That is 540 ms, plus the STFT window. The expected value is about 0.55 s.

Not checked: the STFT call at `:254-255` uses the torch default for `center`. Edge padding can change the output near the start and the end of a signal. Keep the target region away from both edges.

## Why this does not block MVP

The MVP restores whole files. It needs no lookahead number.

## Expected behavior

1. Restore one fixed input with the stock torch model, as one whole run with no chunks.
2. Change only the samples more than N seconds after a target region. Restore again.
3. Find the smallest N at which the target region stays within 1e-6 of the first run. Record the tolerance if another one is used.
4. Do the same for samples before the target region.
5. Run the same search on the ONNX export of task 01.

## Promotion condition

Promote this record after B-053 admits the live mode and B-054 records GO.

## Acceptance criteria

- [ ] A record in `docs/evidence` holds the measured lookahead and lookback in milliseconds, for torch and for ONNX.
- [ ] The record compares the result with the expected 0.55 s and explains a difference.
- [ ] A script in `tools` reproduces the measurement.
