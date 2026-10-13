# 08 — Add GPU execution with CPU fallback

**Status:** TODO
**Phase:** 1 — Restoration engine
**Depends on:** 07
**SRS:** §6
**Stack:** §C
**Assigned to:** Agent
**Started:** 2026-10-13 (base `9142778`; part 1 ended at `248d315`, part 2 resumed there)
**Outcome:** Windows uses the CUDA execution provider. `Device::Gpu` loads the GPU ONNX Runtime
build `onnxruntime-win-x64-gpu_cuda12-1.28.3` (MIT) through the same `ort` `load-dynamic` path and
registers `CUDAExecutionProvider` ahead of the CPU provider with `error_on_failure`. The CUDA and
cuDNN libraries come from a folder the caller gives: `gpu_provider_available(runtime_library,
cuda_folder)` and `load_onnx_backend_on_device(runtime_library, model, device, cuda_folder)`.
`Device::Auto` picks CUDA only when the libraries preload and the runtime advertises CUDA. A
provider that cannot start returns `ProviderStart`, and the job runs again on the CPU and tells
the user (SRS §6.3). DirectML and WinML are out: DirectML fails on the model's `ConvTranspose`
node and has no 1.28.x build, and WinML's NVIDIA EP `NvTensorRtRtx` needs the Windows ML
`ExecutionProviderCatalog`, which the `ort` C-API builder cannot use. `tools/fetch_cuda.py` pins
the GPU build and the NVIDIA CUDA 12.x / cuDNN 9.x wheels by SHA-256; it is not part of
`pnpm verify`. Measured on the RTX 3060 Laptop (2026-10-14): Rust engine CUDA RTF **0.5785** (18 s
channel), peak device VRAM **5124 MiB**; Rust CPU RTF **2.621** (6 s stereo, 1.310 per
channel-second); WSL2 CUDA (Python, `onnxruntime-gpu` 1.28.0) RTF **0.9836**. Parity within SRS
§12.1: GPU vs CPU max abs ≤ 6.81e-4 / RMSE ≤ 6.30e-5; GPU vs golden max abs ≤ 7.23e-4 / RMSE ≤
6.76e-5. The NVIDIA wheels are proprietary; the license review is a separate MVP task. Gate
`pnpm verify`: TBD.

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

- [x] The Windows GPU path is measured and within parity.
- [x] The CPU fallback is proven by the injected-failure test.
- [x] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core gpu
cargo run --release -p melomae-cli -- restore <fixture> <output> --device auto
pnpm verify
```
