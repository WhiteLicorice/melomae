# B-031 — Measure chunk timing medians and output parity

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 05

## Impact

Task 05 timed each configuration once, cold, so the arena-off real-time factor (+5.8%) is not a median. The 60 s acceptance compares peak memory only, not the output samples against stock Apollo. Both task 05 reviews found this (N1).

## Why this does not block MVP

The 6 s parity test already compares output samples, and the arena change is a session option.

## Expected behavior

Repeated alternating runs with fixed threads give median real-time factors for arena on and off, and the 60 s output matches stock Apollo within the parity tolerance.

## Promotion condition

Promote this task with the next performance work (task 08), or when a user reports slower restores.

## Acceptance criteria

- [ ] A measurement records at least five alternating runs per configuration with the median.
- [ ] A 60 s comparison against stock Apollo passes the parity tolerance.
