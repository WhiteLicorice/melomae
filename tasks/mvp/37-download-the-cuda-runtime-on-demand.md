# 37 — Download the CUDA runtime on demand

**Status:** TODO
**Phase:** 1 — Restoration engine
**Depends on:** 08, 38
**SRS:** §6
**Stack:** §C, §J
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

When the machine has a CUDA-capable NVIDIA GPU, the app downloads the CUDA runtime that the GPU path needs, verifies it, and caches it, with no action from the user.

## Evidence

- The author ruled on 2026-10-14: "Just download CUDA automatically if device is CUDA compatible. 1.5gb is nothing on modern systems." DirectML failed on the model's `ConvTranspose` node, and WinML is not reachable from the `ort` session builder (task 08 `Outcome`).
- The CUDA provider and its libraries do not fit the 150 MB installer budget (Stack §J). Task 08 measured about 1.5 GB: the ONNX Runtime CUDA provider, cuDNN 9, and cuBLAS.
- Task 08 adds `tools/fetch_cuda.py`, which fetches pinned archives with SHA-256 checks for the tests. This task gives the app the same behavior at run time.

## Scope

1. Detect a CUDA-capable NVIDIA GPU and its driver version.
2. Download the pinned archives from task 08, verify each SHA-256, and extract the libraries into the per-user app data folder.
3. Show progress and allow cancel. A failed or cancelled download leaves no partial library set, and the job runs on CPU.
4. Point the GPU device at the cached folder (the task 08 engine takes that folder).
5. Use the download only after task 38 settles the license terms.

## Out of scope

- The device picker UI (task 19).

## Test-first plan

1. A test with a local file server serves the archives. A wrong hash, a cut connection, and a cancel each leave no library set and fall back to CPU.

## Acceptance criteria

- [ ] A CUDA-capable machine with an empty cache runs a GPU restore after the automatic download.
- [ ] A machine with no NVIDIA GPU downloads nothing.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.
