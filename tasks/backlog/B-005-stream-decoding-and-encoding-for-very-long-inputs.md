# B-005 — Stream decoding and encoding for very long inputs

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 04, 05

## Impact

Task 04 holds the full decoded input and two full-length accumulators in memory. That is about three times the input's PCM size. A 10-minute stereo input needs about 640 MB. A 60-minute mix needs about 3.8 GB.

## Why this does not block MVP

Typical tracks are under 10 minutes. Adaptive chunks (task 05) bound the model's memory, which is the largest cost.

## Expected behavior

The engine decodes, infers, and encodes as a stream. Memory stays flat regardless of input length. The output stays identical to task 04's output.

## Promotion condition

Promote this task when a user runs out of memory on a long input, or when DJ mixes and audiobooks become a target.

## Acceptance criteria

- [ ] Peak memory for a 60-minute input stays within 10% of the peak for a 10-minute input.
- [ ] The output matches the non-streamed output exactly.
