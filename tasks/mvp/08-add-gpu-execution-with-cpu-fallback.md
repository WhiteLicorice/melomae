# 08 — Add GPU execution with CPU fallback

**Status:** DONE
**Phase:** 1 — Restoration engine
**Depends on:** 07
**SRS:** §6
**Stack:** §C
**Assigned to:** Agent
**Started:** 2026-10-13 (base `9142778`; part 1 ended at `248d315`, part 2 resumed there; rebased onto `master` `b5ec0c4` on 2026-11-18)
**Outcome:** Windows uses the CUDA execution provider. `Device::Gpu` loads the GPU ONNX Runtime
build `onnxruntime-win-x64-gpu_cuda12-1.28.3` (MIT) through the same `ort` `load-dynamic` path and
registers `CUDAExecutionProvider` ahead of the CPU provider with `error_on_failure`. The CUDA and
cuDNN libraries come from a folder the caller gives: `gpu_provider_available(runtime_library,
cuda_folder)` and `load_onnx_backend_on_device(runtime_library, model, device, cuda_folder)`.
`Device::Auto` picks CUDA only when the libraries preload and the runtime advertises CUDA. A
provider that cannot start returns `ProviderStart`, and the job runs again on the CPU and tells the
user (SRS §6.3). DirectML and WinML are out: DirectML fails on the model's `ConvTranspose` node and
has no 1.28.x build, and WinML's NVIDIA EP `NvTensorRtRtx` needs the Windows ML
`ExecutionProviderCatalog`, which the `ort` C-API builder cannot use. `tools/fetch_cuda.py` pins the
GPU build and the NVIDIA CUDA 12.x / cuDNN 9.x wheels by SHA-256; it is not part of `pnpm verify`.
Its first real run (2026-11-18) downloaded all six archives and re-hashed each to its pinned
SHA-256; `.cache/cuda/v1.28.3-cuda12/lib` holds 21 DLLs.

The pinned folder alone was not enough. The provider started, but every run failed at the first
`ConvTranspose` node with `CUDNN_STATUS_SUBLIBRARY_LOADING_FAILED`, because the cuDNN 9 frontend
loads `cudnn_engines_tensor_ir64_9.dll` by bare name and Windows does not search the folder of the
loaded module. `preload_cuda_dylibs` now puts the CUDA folder on the process DLL search path once
(`crates/melomae-core/src/inference/onnx.rs:118`, helper at `:137`). Measured on the RTX 3060
Laptop: Rust engine CUDA RTF **0.5785** (18 s channel), peak device VRAM **5124 MiB**; Rust CPU RTF
**2.621** (6 s stereo, 1.310 per channel-second); WSL2 CUDA (Python, `onnxruntime-gpu` 1.28.0) RTF
**0.9836**. Post-fix parity against the SRS §12.1 limits of 1e-3 (max abs) and 1e-4 (RMSE): GPU vs
CPU max abs 4.91e-4 and 5.49e-4 (channels 0 and 1), RMSE 5.71e-5 and 5.67e-5; GPU vs golden max abs
5.94e-4 and 6.75e-4, RMSE 6.20e-5 and 6.02e-5. `melomae-cli restore --device auto` on the 6 s
parity fixture chose the GPU and ran at RTF 0.883 (5.3 s). The NVIDIA wheels are proprietary; the
license review is a separate MVP task. Gate `pnpm verify` at `eeade99` (rebased onto `b5ec0c4`):
exit 0 — board 122 records, tasks unittest 12 passed, Vitest 1 passed, cargo test 291 passed / 5
ignored (35 suites), pytest 45 passed.

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
- [x] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core gpu
cargo run --release -p melomae-cli -- restore <fixture> <output> --device auto
pnpm verify
```
