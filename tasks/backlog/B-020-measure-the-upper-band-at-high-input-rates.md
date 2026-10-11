# B-020 — Measure the upper band at high input rates

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 03

## Impact

The task 03 resampler gate measures a 48 kHz input against an analytic reference: 105.8 dB through 19933 Hz and 100.4 dB for a steady 20 kHz tone. A review diagnostic measured a steady 20 kHz tone at 62.9 dB for an 88.2 kHz input and at 54.7 dB for a 96 kHz input. No gate covers those rates.

## Why this does not block MVP

The settled gate is the 48 kHz case. Most music sources are 44.1 or 48 kHz. 20 kHz is at the edge of hearing.

## Expected behavior

The resampler holds the settled 80 dB bound at 20 kHz for 88.2 and 96 kHz inputs, or the limit is documented with its cause.

## Promotion condition

Promote this task with high-resolution source support, or with the next change to `decode/resample.rs`.

## Acceptance criteria

- [ ] A steady 20 kHz tone gate runs for 88.2 and 96 kHz inputs.
- [ ] The gate passes at 80 dB, or the task records the measured limit and the reason.
