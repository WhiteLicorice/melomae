# R-003 — Merge the three pinned-download scripts

**Status:** TODO
**Severity:** Refactor
**Depends on:** 00

## Impact

Three tool scripts each define the same six functions for a pinned download: the platform lookup, the hash, the completeness check, the extract step, the download, and the install. A correction must go into each copy.

## Evidence

The ponytail audit of 2026-11-11 read the code at `master` `23f0bcf`. It built and ran nothing. The orchestrator read each cited line again on the same day.

- `tools/fetch_ffmpeg.py:49-134`, `tools/fetch_onnxruntime.py:63-149`, and `tools/cargo_deny.py:59-140` each define `platform_asset`, `sha256_of`, `is_complete`, `extract`, `download`, and `install`.
- The audit reports that the three `install` functions match line for line. The orchestrator compared the function lists only.

## Why this is a refactor

Each script works. The change removes duplicate code and changes no download.

## Expected behavior

One module, for example `tools/pinned_fetch.py`, holds the shared steps. Each script keeps its pin table and its binary lookup.

## Promotion condition

Promote this task before the next pinned download is added, or with B-068.

## Smallest correct fix boundary

- Keep each script name and its command line, because `package.json` calls them.
- Do not change a pin or a checksum.

## Test-first plan

- The gate is the characterization test. The tests in `tools/tests` for the three scripts must pass with no edit to their assertions.

## Acceptance criteria

- [ ] One definition of `install`, `sha256_of`, and `download` exists.
- [ ] The tool tests pass with no change to their assertions.
- [ ] A fresh clone still downloads all three assets.
