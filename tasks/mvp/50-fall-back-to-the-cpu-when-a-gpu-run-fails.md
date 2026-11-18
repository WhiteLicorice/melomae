# 50 — Fall back to the CPU when a GPU run fails

**Status:** TODO
**Phase:** 1 — Restoration engine
**Depends on:** 08
**SRS:** §6
**Stack:** §C
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Retry a job on the CPU when the GPU fails during inference, not only when the provider fails to start, as SRS §6.3 says.

## Evidence

- The task 08 review (2026-11-18, `C:\mt\reports\08-review.md` findings 1 and 2) found that every `session.run` error becomes `InferenceError::Model` (`crates/melomae-core/src/inference/onnx.rs:199-201`, `:217-219`). `falls_back_to_cpu` matches only `ProviderStart` and `OutOfMemory` (`inference/mod.rs:212-214`).
- With the CUDA DLL-path fix removed, the GPU e2e test fails at the first `ConvTranspose` with `Model("... CUDNN_STATUS_SUBLIBRARY_LOADING_FAILED")`. The job exits and does not retry on the CPU.
- Production never constructs `OutOfMemory`. Only two test fakes do (`inference/device.rs:328`, `:452`). So the out-of-memory retry in task 08 scope 4 never runs.

## Scope

1. Classify a GPU `session.run` failure into a fallback-eligible error. Classify an ONNX Runtime allocation failure into `OutOfMemory`.
2. Include both in `falls_back_to_cpu`. The job tells the user that it retried on the CPU.

## Out of scope

- A CPU run failure. It stays a job error.

## Test-first plan

1. A device test injects a GPU run failure. It fails before the fix because the error is returned.
2. An e2e test injects a run failure on `--device gpu` and expects the CPU result and the fallback report.

## Acceptance criteria

- [ ] A GPU run failure and a GPU out-of-memory failure both retry on the CPU and report it.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.
