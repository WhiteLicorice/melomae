# B-069 — Run the GPU path on Linux

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 08

## Impact

On Linux, `--device gpu` has no CUDA path. A Linux user with an NVIDIA GPU runs on the CPU.

## Evidence

- The author moved the Linux GPU path to the backlog on 2026-10-14. Task 08 built and measured only the Windows CUDA path.
- WSL2 can serve as a partial test bed. The 2026-10-14 probe installed `onnxruntime-gpu` 1.28.0 in a user-space venv, but the Rust GPU path was never built there.

## Why this does not block MVP

The MVP GPU target is Windows. Linux keeps the CPU path, which task 08 proves as the fallback.

## Expected behavior

`--device gpu` on Linux with CUDA uses the CUDA provider and falls back to the CPU as on Windows.

## Promotion condition

A Linux release is planned, or a Linux user reports the CPU speed as a problem.

## Acceptance criteria

- [ ] The Linux CUDA path builds and passes the GPU parity test on a Linux CUDA machine.
- [ ] `pnpm verify` passes.
