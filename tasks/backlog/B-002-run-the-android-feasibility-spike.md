# B-002 — Run the Android feasibility spike

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 01, 04

## Impact

The briefer names Android as exploratory and gates it on a feasibility spike (`docs/source/briefer.md`, "Stretch scope").

## Evidence

- The model costs about 250 GFLOP per channel-second. This figure is inferred from the layer shapes in `C:\Lab\Apollo\look2hear\models\apollo.py:73-80,149-153,174-177`.
- The x86 dev machine delivers about 185 GFLOP/s in torch CPU (6 channel-seconds in 8.09 s). A mid-range phone CPU is likely many times slower. This is inferred, not measured.
- Task 01's ONNX model is the input for ONNX Runtime Mobile.

## Why this does not block MVP

SRS §2 makes Android exploratory. The briefer says: do not build any Android app before the spike.

## Expected behavior

The spike is time-boxed to two working days. It runs the task 01 model with ONNX Runtime Mobile on one named mid-range phone. It measures RTF, peak memory, and thermal throttling on a 30 s stereo input. It tests the NNAPI or GPU delegates where they exist. It ends with `GO` or `NO-GO`.

## Promotion condition

Promote this task when the owner names a target phone and wants the Android question answered.

## Acceptance criteria

- [ ] RTF, peak memory, and thermal numbers on the named phone.
- [ ] A `GO` or `NO-GO` with the reason.
- [ ] No Android app code exists before `GO`.
