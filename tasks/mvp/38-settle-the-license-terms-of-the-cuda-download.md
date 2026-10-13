# 38 — Settle the license terms of the CUDA download

**Status:** TODO
**Phase:** 5 — Packaging and release
**Depends on:** 08
**SRS:** §11
**Stack:** §C, §J
**Assigned to:** Owner
**Started:** —
**Outcome:** —

## Goal

Decide, and record in the repository, the terms under which the app downloads and uses the NVIDIA CUDA libraries, so that task 37 does not ship on an open license question.

## Evidence

- The author ruled on 2026-10-14 that the app downloads CUDA automatically on a CUDA-capable device (task 37).
- The download holds NVIDIA binaries (CUDA runtime, cuBLAS, cuDNN), under NVIDIA's license terms, and the ONNX Runtime CUDA provider, under the MIT license. Task 08 `Outcome` records each component, its source URL, and its license.
- Melomae is GPL-3.0-only (SRS §11.1). The app loads the NVIDIA libraries at run time and does not bundle them in the installer.

## Scope

1. Read the license of each downloaded component and record its source URL and date.
2. Decide whether the app may download each component from its source at run time, and whether the app must show the user any terms first.
3. Decide how the GPL-3.0-only app relates to the NVIDIA libraries that it loads at run time.
4. Record the decisions in `docs/melomae-srs.md` §11, and in the third-party notices if a component needs a notice.

## Acceptance criteria

- [ ] Each downloaded component has a recorded license, source, and decision.
- [ ] SRS §11 records the decisions.
