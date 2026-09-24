# 08 — Add GPU execution with CPU fallback

**Status:** TODO
**Phase:** 1 — Restoration engine
**Depends on:** 07
**SRS:** §6
**Stack:** §C
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Use a GPU when one works, and fall back to CPU automatically when it does not.

## Evidence

- torch CUDA on the RTX 3060 Laptop: RTF 0.155 at 6 s, peak VRAM 2.45 GiB (Stack §K).
- DirectML is in maintenance mode. WinML is its successor. Check both states live at execution time.
- WSL2 Ubuntu is installed and can serve as a partial Linux CUDA test bed.

## Scope

1. Measure each candidate execution provider on the RTX 3060 against ONNX Runtime CPU and against torch CUDA:
   - Windows: DirectML (any DirectX 12 GPU, small DLL), CUDA (NVIDIA only, CUDA and cuDNN downloaded on demand), and WinML.
   - Linux: CUDA, downloaded on demand. Use WSL2 as a partial test bed.
2. Pick one provider per OS. Record the numbers and the reason in `Outcome`.
3. Enforce parity with CPU within the §12.1 tolerances.
4. If the provider fails to start or runs out of memory, retry the job on CPU and tell the user.
5. Size GPU chunks from the measured VRAM per second. If the app cannot query VRAM, use a fixed conservative chunk.
6. Add the device values Auto, CPU, and GPU to the core and to `melomae-cli --device`.

An allowed outcome: Linux GPU moves to the backlog, with the evidence recorded.

## Out of scope

- The device picker UI (task 19)

## Test-first plan

1. An injected provider-start failure falls back to CPU and reports the fallback.
2. A GPU parity test runs when a GPU is present. It skips only when no GPU exists, and the skip message says so.

Run the tests. They fail because the fallback does not exist.

## Acceptance criteria

- [ ] The Windows GPU path is measured and within parity.
- [ ] The CPU fallback is proven by the injected-failure test.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core gpu
cargo run --release -p melomae-cli -- restore <fixture> <output> --device auto
pnpm verify
```
