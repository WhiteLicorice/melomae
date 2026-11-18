# 52 — Test the CUDA download script

**Status:** TODO
**Phase:** 1 — Restoration engine
**Depends on:** 08
**SRS:** §6
**Stack:** §C
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Cover `tools/fetch_cuda.py` with tests, as the ffmpeg and ONNX Runtime fetchers already are.

## Evidence

- The task 08 review (finding 7) found no `tools/tests/test_fetch_cuda.py`. Only `test_fetch_ffmpeg.py` and `test_fetch_onnxruntime.py` exist.
- The checksum check (`tools/fetch_cuda.py:132-149`) ran only on its first real download.

## Scope

1. Add `tools/tests/test_fetch_cuda.py`, with the injected fetch and a temporary folder: a matching hash reuses the archive, a wrong hash raises `ChecksumError` and extracts nothing, and the `.complete` marker skips the work.

## Acceptance criteria

- [ ] The three cases pass, and a wrong hash fails before any extraction.
- [ ] `pnpm verify` passes.
