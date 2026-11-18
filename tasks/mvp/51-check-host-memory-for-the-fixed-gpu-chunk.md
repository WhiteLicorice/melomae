# 51 — Check host memory for the fixed GPU chunk

**Status:** TODO
**Phase:** 1 — Restoration engine
**Depends on:** 08
**SRS:** §6
**Stack:** §C
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Refuse a GPU job whose whole-file reserve does not fit in host memory, as the CPU chooser already does (SRS §6.4).

## Evidence

- The task 08 review (finding 6) found that when the video memory is unknown, `choose_gpu_chunk` returns the fixed 6 s chunk without a host-memory check (`crates/melomae-core/src/inference/chunk_budget.rs:194-203`). `choose_chunk` checks `reserve_bytes + peak <= budget_bytes` and returns `InsufficientMemory` (`:231-240`).
- The shipping CLI takes that branch. `08-cli-device.txt` prints `budget 0 bytes, available 0 bytes`.

## Scope

1. In the unknown-memory branch, check the reserve and the predicted GPU peak against the host-memory budget the CPU chooser uses. Return `InsufficientMemory` when it does not fit.

## Test-first plan

1. A unit test gives a reserve larger than the budget and expects `InsufficientMemory`. It fails before the fix.

## Acceptance criteria

- [ ] A GPU job that cannot fit in host memory stops with `InsufficientMemory`.
- [ ] The new test failed first.
- [ ] `pnpm verify` passes.
