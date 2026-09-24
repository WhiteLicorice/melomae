# B-004 — Offer a faster reduced-precision model

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 01, 04

## Impact

On CPU, a 4-minute song takes about 12.5 min on the dev machine (Stack §K). An fp16 or int8 model could be faster.

## Evidence

The SRS §12.1 parity tolerance is tight: max abs 1e-3 and RMSE 1e-4. Reduced precision may exceed it.

## Why this does not block MVP

The fp32 model meets the CPU requirement. Reduced precision puts output parity at risk.

## Expected behavior

The app offers a "fast" model only when it passes the §12.1 tolerances, or passes a documented listening test that the owner approves. The setting is opt-in. The default stays fp32.

## Promotion condition

Promote this task when CPU users report that restore time blocks their use of the app.

## Acceptance criteria

- [ ] RTF and parity numbers for each candidate precision.
- [ ] The owner's decision on any model that misses §12.1.
