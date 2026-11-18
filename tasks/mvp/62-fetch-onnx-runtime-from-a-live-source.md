# 62 — Fetch ONNX Runtime from a live source

**Status:** TODO
**Phase:** 1 — Restoration engine
**Depends on:** —
**SRS:** §6
**Stack:** §C
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Make a fresh clone fetch the pinned ONNX Runtime 1.28.3 builds again.

## Evidence

- 2026-11-18, from this machine: `https://github.com/microsoft/onnxruntime` returned HTTP 404, and `https://api.github.com/repos/microsoft/onnxruntime` returned `"message": "Not Found"`. `https://github.com/` and `https://github.com/microsoft/vscode` returned 200 at the same time.
- The three pinned assets under `releases/download/v1.28.3/` returned 404: `onnxruntime-win-x64-1.28.3.zip`, `onnxruntime-linux-x64-1.28.3.tgz`, and `onnxruntime-win-x64-gpu_cuda12-1.28.3.zip`.
- `tools/fetch_onnxruntime.py:53` and `tools/fetch_cuda.py:56` build their URLs on that repository. Existing worktrees still run from their `.cache` copies, so the gate stays green on this machine and hides the break.

## Scope

1. Find where the 1.28.3 builds are published now, from a primary source. Confirm each pinned SHA-256 against the new download before any URL changes.
2. Point both fetchers at that source. Keep the pinned hashes.
3. If no source serves byte-identical archives, stop and record the options for the owner.

## Acceptance criteria

- [ ] Both fetchers download into an empty `.cache` and verify the pinned hashes.
- [ ] `pnpm verify` passes from a fresh clone with no `.cache`.
