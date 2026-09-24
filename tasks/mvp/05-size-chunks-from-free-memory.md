# 05 — Size chunks from free memory

**Status:** TODO
**Phase:** 1 — Restoration engine
**Depends on:** 04
**SRS:** §6, §12
**Stack:** §C, §K
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Pick the largest chunk that keeps peak inference memory within budget on the current machine. The owner chose adaptive sizing on 2026-09-24.

## Evidence

- torch CPU peak memory grows by about 0.29 GiB per channel-second (Stack §K). Task 01 measured the ONNX Runtime value. Use the task 01 value.
- The compute overhead is (c + 2p) / (c − o). With p = 1 s and o = 1 s, a 4 s chunk costs 2.0× and a 20 s chunk costs 1.16×.

## Scope

1. Model the peak memory as a baseline plus the bytes per channel-second from task 01, times (chunk + 2 × pad).
2. Read the available RAM with the `sysinfo` crate at job start.
3. Budget: at most 50% of the available RAM.
4. Chunk range: 4 s to 30 s. Pad 1 s. Overlap 1 s.
5. Pick the largest chunk inside the budget.
6. If even a 4 s chunk does not fit, fail with a message that names the memory needed and the memory available.
7. Log the chosen chunk on the job.

## Out of scope

- GPU chunk sizing (task 08)

## Test-first plan

1. Write pure-function tests with injected available memory of 2, 4, 8, and 16 GiB. Assert the chosen chunk for each.
2. Write a below-floor test that asserts the error message names both memory values.

Run the tests. They fail because the function does not exist.

## Acceptance criteria

- [ ] A restore of a 60 s fixture peaks within ±15% of the predicted value. The numbers are in `Outcome`.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core chunk_budget
pnpm verify
```
