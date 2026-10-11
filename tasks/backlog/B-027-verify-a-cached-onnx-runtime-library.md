# B-027 — Verify a cached ONNX Runtime library

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 33

## Impact

`tools/fetch_onnxruntime.py` (lines 92 and 124) returns an existing install when its marker file and the library exist. It does not check the marker's asset and hash or the library bytes. A corrupted cache passes. Both task 33 reviews found this (N1). Fresh downloads are verified.

## Why this does not block MVP

The cache is local to a developer or a CI runner, and a corrupted cache needs an outside fault.

## Expected behavior

The script checks the marker against the current pin and the extracted library's hash, and fetches again on a mismatch.

## Promotion condition

Promote this task when a cached-library fault appears, or with the installer work (tasks 27 and 28).

## Acceptance criteria

- [ ] A test with a corrupted cache and a wrong marker fetches again, and fails before the change.
