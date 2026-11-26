# 63 — Move to ONNX Runtime 1.31 and ort rc.15

**Status:** IN PROGRESS
**Phase:** 1 — Restoration engine
**Depends on:** 62
**SRS:** §6
**Stack:** §C
**Assigned to:** Agent
**Started:** 2026-11-26. Base `619ce69` on `task/62`, a clean tree. Gate at the base: `pnpm verify` exit 0 on 2026-11-25 (board 137 records; unittest 12 passed; Vitest 6 passed; cargo 307 passed, 0 failed, 5 ignored; pytest 49 passed).
**Outcome:** —

## Owner decisions (2026-11-26)

1. The owner first chose ONNX Runtime 1.32.0 with `ort` 2.0.0-rc.15. The other options were 1.32.0 under `ort` 2.0.0-rc.13, and a return to 1.28.3.
2. The `ort` 2.0.0-rc.15 source then showed that it is built for ONNX Runtime 1.31, not 1.32: its README badge reads `onnxruntime-v1.31.0`, and its newest API feature is `api-31`. With a 1.32 runtime, `ort` logs that it "may have compatibility issues". The owner chose 1.31.1, the newest 1.31 patch, over 1.32.0.
3. `ort` 2.0.0-rc.15 lists the CUDA 13 libraries in `ort::ep::cuda::CUDA_DYLIBS` (`src/ep/cuda.rs:332`). The owner chose to keep the GPU path on CUDA 12, over a move to CUDA 13. A CUDA 13 move needs a newer driver, drops older GPU generations, and touches the task 38 license review.

## Goal

Run the engine on `ort` 2.0.0-rc.15 and the ONNX Runtime release it is built for.

## Evidence

- Task 62 moved the pin to ONNX Runtime 1.28.0, the newest 1.28 build that Microsoft still publishes on nuget.org.
- On 2026-11-26, nuget.org lists `Microsoft.ML.OnnxRuntime` and `Microsoft.ML.OnnxRuntime.Gpu.Windows` up to `1.32.0`, with `1.31.1` before it. Microsoft's `onnxruntime-cuda-12` feed lists `Microsoft.ML.OnnxRuntime.Gpu.Windows` up to `1.32.0`, with `1.31.1` before it.
- crates.io lists `ort` `2.0.0-rc.15` (2026-10-19) with the features `api-29` to `api-31`. `2.0.0-rc.13`, the old pin, stops at `api-28`.
- The rc.15 release notes say it fixes "over 30 instances of undefined behavior" and recommend the upgrade because of memory-safety fixes. The rc.14 notes say a session now needs an `Environment`: `Session::builder(&env)` after `ort::init().build()`.
- The engine called `ort::init_from` (`crates/melomae-core/src/inference/mod.rs:598`) and `Session::builder()` (`crates/melomae-core/src/inference/onnx.rs:64`).

## Scope

1. Pin `ort` `=2.0.0-rc.15` with `api-31`.
2. Move the engine to the rc.15 API. Keep the once-per-process runtime load and its error behavior (`init_runtime`).
3. Keep the CUDA 12 library names in the engine, because `ort` 2.0.0-rc.15 lists the CUDA 13 names.
4. Move both fetchers, the test tags, the notices, and the CI cache key to 1.31.1. Confirm each package hash from the download. Confirm that the CUDA build needs only the pinned CUDA 12 libraries.
5. Update the `ort` version in `docs/melomae-stack.md` §C.

## Acceptance criteria

- [ ] Both fetchers download 1.31.1 into an empty tag folder and verify the pinned hashes.
- [ ] The CPU parity and golden tests and the two `inference_gpu` tests pass on 1.31.1.
- [ ] `pnpm verify` passes.
