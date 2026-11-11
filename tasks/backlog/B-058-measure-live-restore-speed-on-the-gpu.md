# B-058 — Measure live restore speed on the GPU

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** B-057, 08

## Impact

A live mode must restore each block faster than the block plays. If the GPU cannot do that with a margin, the audio drops, and the mode is not usable.

## Evidence

RTF is the wall time divided by the audio time. Confirmed in `docs/melomae-stack.md:113-118` and `docs/evidence/01-onnx-spike.md:81`:

- torch CUDA on the dev machine: RTF 0.244 at 1 s, and 0.155 at 6 s.
- torch CPU, 8 threads: RTF 3.15 at 1 s, and 2.70 at 3 s. A later CPU run measured 1.95.
- The model costs about 250 GFLOP for each channel-second (`docs/melomae-stack.md:125`).

Inferred, not confirmed:

- The CPU cannot run live. It needs a speed gain of 2 to 3 times (B-004).
- The 1 s figure is the only short measurement. A shorter block can cost much more for each second of audio.
- The start delay is the lookahead, plus the block size, plus the compute time, plus the device buffers. The expected value on the dev machine is about 0.7 to 1 s.
- A GPU stall longer than one block causes a dropout. A safety buffer trades delay for fewer dropouts.

## Why this does not block MVP

The MVP has no real-time limit.

## Expected behavior

1. Run the stateful model of B-057 on the GPU path of task 08.
2. For blocks of 0.1, 0.25, 0.5, and 1 s, measure the RTF of stereo audio over at least 10 minutes.
3. Record the mean, the 99th percentile, and the longest block time.
4. Compute the end-to-end delay for each block size from the measured numbers.

The proposed GO limit is RTF 0.5 or less with a delay of 1 s or less. The owner confirms the limit before the run.

## Promotion condition

Promote this record when B-057 and task 08 are DONE.

## Acceptance criteria

- [ ] A record in `docs/evidence` holds the GPU, the driver, the block size, the RTF numbers, and the computed delay for each run.
- [ ] The record states GO or NO-GO against the limit that the owner confirmed.
- [ ] A script in `tools` reproduces the measurement.
