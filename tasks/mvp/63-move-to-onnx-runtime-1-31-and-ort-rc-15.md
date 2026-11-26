# 63 — Move to ONNX Runtime 1.31 and ort rc.15

**Status:** DONE
**Phase:** 1 — Restoration engine
**Depends on:** 62
**SRS:** §6
**Stack:** §C
**Assigned to:** Agent
**Started:** 2026-11-26. Base `619ce69` on `task/62`, a clean tree. Gate at the base: `pnpm verify` exit 0 on 2026-11-25 (board 137 records; unittest 12 passed; Vitest 6 passed; cargo 307 passed, 0 failed, 5 ignored; pytest 49 passed).
**Outcome:** DONE on `ort` 2.0.0-rc.15 and ONNX Runtime 1.31.1, with the GPU path on CUDA 12 (owner decisions below). `ort` 2.0.0-rc.15 builds at most one environment per process, so `init_runtime` now builds the environment once with `ort::init_from(path).build()` and keeps it in the runtime state, and every model load passes it to `Session::builder(&environment)`. The once-per-process load and its typed error on a failed load are unchanged. `ort::ep::cuda::CUDA_DYLIBS` in rc.15 names the CUDA 13 libraries, so `onnx.rs` now holds its own `CUDA_DYLIBS` with the CUDA 12 names that rc.13 used, and preloads them with `ort::ep::cuda::CUDNN_DYLIBS`. `tools/fetch_onnxruntime.py` pins `microsoft.ml.onnxruntime.1.31.1.nupkg` from nuget.org (SHA-256 `6871ccbc…c679a`). `tools/fetch_cuda.py` pins `microsoft.ml.onnxruntime.gpu.windows.1.31.1.nupkg` from the `onnxruntime-cuda-12` feed (SHA-256 `bf578833…034a7f`). Both hashes were measured from the downloads and checked again against the cached packages before this record. The 1.31.1 CUDA provider needs only the pinned CUDA 12 libraries, and the Linux CPU library needs at most `GLIBC_2.28`. The pin moved to 1.31.1 in `ORT_TAG`, `CUDA_TAG` (`v1.31.1-cuda12`), `RUNTIME_VERSION`, the `verify.yml` cache key, and `THIRD_PARTY_NOTICES.md`. The 1.31.1 `ThirdPartyNotices.txt` adds Mbed TLS (Apache-2.0 OR GPL-2.0-or-later, used under Apache-2.0) and `microsoft/cpp_client_telemetry` (Apache-2.0). Red first: the version tests in `test_fetch_onnxruntime.py`, `test_fetch_cuda.py`, and `test_generate_notices.py` failed against the 1.28.0 pins. The two `inference_gpu` tests failed on rc.15's own list with `cudart64_13.dll ... did not load`, and passed after the CUDA 12 list, including GPU against CPU and against the golden within SRS §12.1. The fetchers installed into the new `v1.31.1` and `v1.31.1-cuda12` tag folders. The NVIDIA wheels came from the existing `archives/` folder, because their pins did not change. A CPU real-time-factor A/B between the two builds on this loaded machine gave medians of 6.49 and 6.92, with single runs from 3.5 to 9.2, so the difference is within the noise. Gate `pnpm verify` on `c3c9606` exit 0 in 35 min: board 138 records; unittest 12 passed; Vitest 6 passed; cargo 307 passed, 0 failed, 5 ignored; pytest 49 passed, 1 pre-existing warning. Two earlier gate runs on the same code failed only `e2e_restore_encode_cancel_exits_130_and_leaves_no_output`, at its fixed 300 s deadline (`crates/melomae-cli/tests/restore_signals.rs:374-387`), while the machine was under load. The task 62 code (rc.13 on 1.28.0) failed the same test in the same way at that time (323.65 s), and the test passes alone in 212 s, so the cause is the machine load, not this change. The fixed wall-clock deadline is a pre-existing flaw that makes the test fail on a slow or loaded machine. Not adopted: ONNX Runtime 1.32.0, because `ort` 2.0.0-rc.15 is built for 1.31, and CUDA 13.

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

- [x] Both fetchers download 1.31.1 into an empty tag folder and verify the pinned hashes.
- [x] The CPU parity and golden tests and the two `inference_gpu` tests pass on 1.31.1.
- [x] `pnpm verify` passes.
