# 62 — Fetch ONNX Runtime from a live source

**Status:** DONE
**Phase:** 1 — Restoration engine
**Depends on:** —
**SRS:** §6
**Stack:** §C
**Assigned to:** Agent
**Started:** 2026-11-24. Base `7ff92a2` on `master`, a fresh clone with a clean tree and no `.cache`. The gate cannot run at the base: `python tools/fetch_onnxruntime.py` stops with `HTTP Error 404: Not Found` before any check runs.
**Outcome:** DONE on ONNX Runtime 1.28.0 (owner decision below). `tools/fetch_onnxruntime.py` fetches `microsoft.ml.onnxruntime.1.28.0.nupkg` from nuget.org (SHA-256 `769d1d3e…d5db`) and installs the current platform's `runtimes/<rid>/native/` libraries into `.cache/onnxruntime/v1.28.0/onnxruntime-<rid>-1.28.0/lib/`, with the package's `LICENSE` and `ThirdPartyNotices.txt` beside them. That keeps the `<tag>/<folder>/lib/` layout that `tests/setup/mod.rs` and `tools/generate_notices.py` already read, so only their tags changed. `tools/fetch_cuda.py` fetches `microsoft.ml.onnxruntime.gpu.windows.1.28.0.nupkg` from Microsoft's `onnxruntime-cuda-12` Azure Artifacts feed (SHA-256 `1aa6f7da…3d3b`), and its flattening reads `native/` and skips every `runtimes/<rid>/` other than `win-x64`. The nuget.org GPU package of the same name is a CUDA 13 build: the first gate failed both `inference_gpu` tests with `onnxruntime_providers_cuda.dll ... depends on "cublasLt64_13.dll" which is missing`. The CUDA execution provider page confirms that from 1.27 the PyPI and NuGet GPU packages are CUDA 13 builds and that CUDA 12 builds are published separately. The feed's 1.28.0 provider imports `cudart64_12`, `cublas64_12`, `cublasLt64_12`, `cufft64_11`, and `cudnn64_9`, which match the pinned NVIDIA wheels. The pin moved to 1.28.0 in `ORT_TAG`, `CUDA_TAG` (`v1.28.0-cuda12`), `RUNTIME_VERSION`, the `verify.yml` cache key, and `THIRD_PARTY_NOTICES.md`. The notice changes only in its three version lines, because the 1.28.0 `LICENSE` and `ThirdPartyNotices.txt` match the 1.28.3 texts. Red first: the new `test_fetch_onnxruntime.py` cases (the NuGet pin, a fake package installed for `win-x64` and for `linux-x64`) and the new `test_fetch_cuda.py` cases (the CUDA 12 feed pin, the `win-x64`-only flattening) failed against the old scripts (5 failed), and the CUDA 12 feed pin test failed again before the feed change. Both fetchers then downloaded into an empty `.cache`, verified the hashes, and a second run of each did nothing. The two `inference_gpu` tests pass on the RTX 4060 Laptop GPU, including GPU against CPU and against the golden within SRS §12.1. Gate `pnpm verify` on base `7ff92a2` plus this change, from a fresh clone whose `.cache` came only from these fetchers, exit 0 in 39 min: board 137 records; unittest 12 passed; Vitest 6 passed; cargo 307 passed, 0 failed, 5 ignored across 37 binaries; pytest 49 passed, 1 pre-existing warning. Environment notes for this machine, outside the repository: the shell's `python` was a 32-bit MSYS2 build, which aborted on the 0.7 GB cuDNN wheel, so the fetch and the gate used the 64-bit CPython 3.13. The installed `pnpm` is the standalone `@pnpm/exe` build, which adds its own section to `pnpm-lock.yaml` on every run and then cannot read the file, so the gate ran under the pinned `pnpm@12.6.0` through corepack, with `node_modules` from `--frozen-lockfile`. Not adopted: the `ORT-Nightly` feed also lists `Microsoft.ML.OnnxRuntime` 1.28.1 to 1.28.3, and the CUDA 12 feed lists the GPU package at 1.28.3. Their provider imports both `cudart64_12` and `cudart64_13`, so a later move back to 1.28.3 needs its own check.

## Owner decision (2026-11-24)

The owner chose option 1: fetch NuGet `1.28.0`. The pin moves from 1.28.3 to 1.28.0 and the pinned hashes become the two NuGet package hashes below. Acceptance criterion 1 reads against those hashes.

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

- [x] Both fetchers download into an empty `.cache` and verify the pinned hashes.
- [x] `pnpm verify` passes from a fresh clone with no `.cache`.

## Search (2026-11-24)

- `https://github.com/microsoft/onnxruntime` and the pinned `releases/download/v1.28.3/onnxruntime-win-x64-1.28.3.zip` still return 404. `https://github.com/onnx/onnxruntime` returns 404. A GitHub repository search finds no `onnxruntime` repository under `microsoft` or `onnx`.
- `https://onnxruntime.ai/docs/install/` returns 200, and its download links still point at `github.com/microsoft/onnxruntime/releases`.
- The 1.28 patch releases were published on GitHub only. NuGet (`Microsoft.ML.OnnxRuntime`, `Microsoft.ML.OnnxRuntime.Gpu`, `Microsoft.ML.OnnxRuntime.Gpu.Windows`), PyPI (`onnxruntime`, `onnxruntime-gpu`), and Maven Central (`com.microsoft.onnxruntime:onnxruntime`) carry `1.28.0` and no 1.28.1 to 1.28.3. Their newest version is `1.32.0` (PyPI upload 2026-11-13).
- The Wayback Machine holds the `v1.28.3` release page (capture `20261003050629`, a patch on top of 1.28.2 with memory-safety and model-validation fixes) but no capture of any `releases/download/v1.28.3/` asset.
- A GitHub code search for the pinned win-x64 SHA-256 finds other projects with the same pin. Each one downloads from the same dead `github.com/microsoft/onnxruntime` URL, so none is a mirror.

## Owner options

Recommended: option 1.

1. **NuGet `1.28.0`, Microsoft-published.** Use `Microsoft.ML.OnnxRuntime` 1.28.0 for the CPU builds of both platforms and `Microsoft.ML.OnnxRuntime.Gpu.Windows` 1.28.0 for the CUDA build. Both are signed packages on nuget.org. They stay on the 1.28 minor line and ORT API 27, so the `ort` pin (`=2.0.0-rc.13`, `api-27`) does not change. Measured from the downloads:
   - `microsoft.ml.onnxruntime.1.28.0.nupkg`: 139145017 bytes, SHA-256 `769d1d3ea8ab6cd69f737c9dd4d4462aa4ad0ccfa106eaf506efc40d7bead5db`. It holds `runtimes/win-x64/native/onnxruntime.dll` and `runtimes/linux-x64/native/libonnxruntime.so`, plus `LICENSE` and `ThirdPartyNotices.txt` at the root, which `tools/generate_notices.py` reads.
   - `microsoft.ml.onnxruntime.gpu.windows.1.28.0.nupkg` on nuget.org: 234399144 bytes, SHA-256 `6a0748af6f6bd5c87c5f948932759d156d08e964c493e48371e3a820f9c3f070`. It holds `onnxruntime.dll`, `onnxruntime_providers_cuda.dll`, and `onnxruntime_providers_shared.dll` under `runtimes/win-x64/native/`. Correction found during the work: this is a CUDA 13 build (see Outcome), so the pin uses the CUDA 12 build of the same package from Microsoft's `onnxruntime-cuda-12` feed instead.
   - The Linux library's newest glibc requirement is `GLIBC_2.27`, and it holds no `__isoc23_*` symbol, so the glibc 2.35 baseline of SRS §2 holds. Both libraries carry the version string `1.28.0`.
   - Costs: the 1.28.1 to 1.28.3 fixes are lost. The 1.28.3 fixes harden shape inference and graph validation against malformed models. Melomae loads only its bundled `apollo.onnx`, so the exposure is low. The parity and golden tests must pass again on 1.28.0. The fetchers must read the NuGet layout (`runtimes/<rid>/native/`) instead of `<name>/lib/`. The pin moves to 1.28.0 in `tools/fetch_onnxruntime.py`, `tools/fetch_cuda.py`, `tools/generate_notices.py`, `crates/melomae-core/tests/setup/mod.rs` (`ORT_TAG`), `crates/melomae-core/tests/inference_gpu.rs` (`CUDA_TAG`), the `verify.yml` cache key, and `THIRD_PARTY_NOTICES.md`.
2. **Rehost the exact 1.28.3 archives.** Upload the original archives as release assets of this repository, and keep every pinned hash. ONNX Runtime is MIT, so redistribution is allowed with its notice. Only the base URLs change. It needs someone who still holds the original archives. `fetch_onnxruntime.py` deletes its archive after it extracts, so the CPU archives are not in a worktree's `.cache`. `fetch_cuda.py` keeps `.cache/cuda/archives/`, so the CUDA archive may still exist there. Re-zipping an extracted folder does not reproduce the pinned hash.
3. **Move to `1.32.0` on NuGet.** Pair it with a newer `ort` release (`2.0.0-rc.15` is the newest on crates.io, 2026-10-19). This changes the engine's API pin and belongs in its own task, not in this one.
4. **Wait for Microsoft to restore the repository.** No code changes, but a fresh clone cannot run the gate until then, and no date is known.
