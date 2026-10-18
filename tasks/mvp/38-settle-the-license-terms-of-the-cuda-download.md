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
- Research on 2026-10-18 read these live sources:
  - ONNX Runtime v1.28.3 on GitHub has two GPU builds: `onnxruntime-{win,linux}-x64-gpu_cuda13-1.28.3` and `gpu_cuda12`. The ONNX Runtime CUDA provider page lists CUDA 13.0 and cuDNN 9.x as the default for 1.28.x.
  - The `gpu_cuda13` Windows zip holds `onnxruntime.dll`, `onnxruntime_providers_cuda.dll`, `onnxruntime_providers_shared.dll`, `onnxruntime_providers_tensorrt.dll`, `LICENSE` (MIT), and `ThirdPartyNotices.txt` (331175 bytes). It holds no NVIDIA binary.
  - NVIDIA publishes the CUDA libraries at `https://developer.download.nvidia.com/compute/cuda/redist/` (manifest `redistrib_13.4.3.json`, release date 2026-10-13). It publishes cuDNN at `https://developer.download.nvidia.com/compute/cudnn/redist/` (manifest `redistrib_9.27.0.json`, release date 2026-09-29). Each manifest gives the archive path, size, SHA-256, and `license_path` for each component.
  - The `LICENSE.txt` in the CUDA archives is the CUDA Toolkit EULA, "Last updated: January 12, 2024". The web page `https://docs.nvidia.com/cuda/eula/` shows v13.4, January 26, 2026. The archive text and the web text are different versions.
  - The cuDNN `LICENSE.txt` is the NVIDIA SDK License Agreement and the cuDNN Supplement, "v. February 22, 2022". Supplement §2 makes "the runtime files .so and .dll" distributable. Supplement §4.1 licenses the SDK "only for use in systems with NVIDIA GPUs".
  - The CUDA EULA §1.2(5) and the cuDNN SLA §2.5 both say: "You may not use the SDK in any manner that would cause it to become subject to an open source software license."
  - Both NVIDIA agreements say that a person who does not accept the terms must not download or use the software. The user who downloads the files is the licensee.
  - GPLv3 §1 (`LICENSE`) excludes only "System Libraries" from the Corresponding Source. These are libraries in the normal packaging of a Major Component (kernel, window system, compiler). The CUDA runtime, cuBLAS, and cuDNN are not part of the operating system or the display driver. The GNU GPL FAQ entry `GPLIncompatibleLibs` says that a program that links a library outside that exception needs a permission from its copyright holders. It gives a GPLv3 §7 template.
  - SRS §11.1 records the owner as the only human author. `THIRD_PARTY_NOTICES.md` lists no GPL-only dependency. `r-efi` is `MIT OR Apache-2.0 OR LGPL-2.1-or-later`.
  - The archive sizes suggest that the Windows download is larger than the 1.5 GB that task 08 measured: cuDNN 9.27.0 `cuda13` is 1330 MB, cuBLAS 13.8.0.4 is 422 MB, and the ONNX Runtime `gpu_cuda13` zip is 349 MB.
- The author ruled on 2026-10-18:
  - Decision 1: add a GPLv3 §7 additional permission for the NVIDIA CUDA libraries.
  - Decision 2: the app downloads each component from its original source. Before the first download, the app shows the NVIDIA terms one time. If the user declines, the job runs on CPU.

## Decisions

1. **Additional permission.** Melomae grants a GPLv3 §7 additional permission. It lets a person convey Melomae combined with the NVIDIA CUDA runtime, cuBLAS, cuBLASLt, cuDNN, and any other NVIDIA library that the ONNX Runtime CUDA provider loads. The GPL does not apply to those NVIDIA libraries. This satisfies CUDA EULA §1.2(5) and cuDNN SLA §2.5.
   - Rejected: no permission, on the argument that the user makes the combination in private use (GPLv3 §2). The FSF treats a program designed for a library as a combined work, and a fork or distribution that conveys Melomae would have no clear permission.
2. **Original sources and a consent screen.** The app downloads the NVIDIA archives from `developer.download.nvidia.com` and the ONNX Runtime GPU build from the Microsoft GitHub release. Melomae does not host or mirror any NVIDIA file. Before the first download, the app shows one screen that names each NVIDIA agreement and links to it. The user accepts or declines. On decline, the app downloads nothing and the job runs on CPU. The app records the choice and does not ask again.
   - Rejected: a silent download with the terms on the attribution screen only, because the user would accept terms that the user never saw.
   - Rejected: Melomae hosts the files under the redistribution grant. CUDA EULA §1.1.2(5) then requires Melomae's terms to be consistent with NVIDIA's terms. Melomae would also have to host about 2 GB for each release.
3. **Notices.** Melomae does not distribute the NVIDIA files, so `THIRD_PARTY_NOTICES.md` does not list them (SRS §11.6 covers bundled components). The attribution screen (task 25) names each downloaded NVIDIA component and links its agreement.

## Scope

1. List the NVIDIA libraries that the pinned ONNX Runtime CUDA provider loads. Read the import table of `onnxruntime_providers_cuda.dll` (`dumpbin /dependents`) and of `libonnxruntime_providers_cuda.so` (`readelf -d`). Do not guess the list from the documentation. The documentation does not give it for 1.20 and later.
2. Confirm that the CUDA major version pinned in task 08 matches the ONNX Runtime GPU build (`gpu_cuda13` or `gpu_cuda12`). Use the matching cuDNN variant from the manifest. If task 08 pinned CUDA 12, record why.
3. For each component, record the name, version, archive URL, SHA-256, license name, and license version. Read the license version from the `LICENSE.txt` inside the pinned archive, not from the web page. Record the date of the reading.
4. Write the §7 additional permission from the FSF template. Put it in a `LICENSE-EXCEPTION` file at the repository root, and name that file in `README.md` and in the `license` notes of `Cargo.toml`. Do not change the SPDX identifier `GPL-3.0-only` without a ruling.
5. Record Decisions 1 to 3 in `docs/melomae-srs.md` §11 as §11.12, in the form of §11.7 to §11.11: the decision, the sources, and the rejected options.
6. Amend task 37: replace "with no action from the user" with the consent screen of Decision 2, and add a test for the decline path.
7. Amend task 25: add the NVIDIA components to the attribution screen, shown only when the download is accepted.
8. Compare the GPU `ThirdPartyNotices.txt` with the CPU one that `THIRD_PARTY_NOTICES.md` reproduces. If they are different, decide which file the notices must carry and record the decision.

## Out of scope

- The download, verification, and cache code (task 37).
- The consent screen UI (task 37 and task 19).

## Open risk

- The §7 permission must also cover the ported Apollo code (SRS §11.11). That code uses GPL-3.0-only as its CC BY-SA 4.0 Adapter's License. GPLv3 §7 says that a permission for the whole Program is "treated as though [it was] included in this License", and any person who conveys the work can remove it. So the permission should keep the license a BY-SA Compatible License. This is an inference. The owner must confirm it before step 4, or limit the permission to the files that the owner wrote.

## Acceptance criteria

- [ ] Each downloaded component has a recorded license, license version, source URL, SHA-256, and decision.
- [ ] The provider's NVIDIA dependencies come from its import tables, and the record names the command.
- [ ] SRS §11.12 records Decisions 1 to 3 and the open risk.
- [ ] `LICENSE-EXCEPTION` holds the §7 additional permission, and `README.md` names it.
- [ ] Task 37 and task 25 are amended to match Decision 2 and Decision 3.
- [ ] `pnpm verify` passes.
