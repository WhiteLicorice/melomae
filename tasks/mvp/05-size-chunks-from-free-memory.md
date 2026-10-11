# 05 — Size chunks from free memory

**Status:** DONE
**Phase:** 1 — Restoration engine
**Depends on:** 04
**SRS:** §6, §12
**Stack:** §C, §K
**Assigned to:** Agent
**Started:** 2026-10-11 — Base commit `af021d8`, clean tree. Baseline gate at the merged task 04 tree: board 56 records, tasks unittest 9 passed, Vitest 1 passed, cargo test 144 passed, pytest 14 passed.
**Outcome:** DONE 2026-10-11. `inference::chunk_budget` picks the largest chunk in 4 s to 30 s whose predicted peak fits half the available memory. `choose_chunk` is pure and takes the available memory, so the tests inject it. `choose_chunk_for_machine` reads the memory with `sysinfo` 0.39.6, license MIT. The peak is `206,745,941 bytes + 541,471,232 bytes x (chunk + 2 x pad)`, fitted on the shipping engine path. The result type carries the chunk, the predicted peak, the budget, and the available memory, and its `Display` names all four for the job log. Task 05 found that the ONNX Runtime CPU memory arena grew the peak from about 11 GB after the first chunk to 22 GB after the last on a 60 s input, so the engine now loads the session with the arena off and the memory pattern off (`OnnxOptions`). The runtime is ONNX Runtime 1.28.0, from `ort-sys` 2.0.0-rc.13. Task 01 used 1.30.0. Diagnosis, chunk 17 s on the 60 s input, one-channel prediction 11,380,000,000 B: (a) stereo arena on, peak 22,318,972,928 B, +96.1 percent; (b) stereo arena off, peak 10,493,063,168 B, -7.8 percent; (c) mono arena on, peak 22,299,820,032 B, +95.9 percent. The arena-off peak is flat after the first chunk. Mono doubles too, so the growth is per run, not per channel. Real-time factor on the 6 s parity input: 2.5289 arena off against 2.3903 arena on, +5.8 percent, under the 10 percent limit. Fit points on the shipping path (chunk, padded seconds, measured, fit): 4 s, 6, 3,456,028,672, 3,455,573,333; 8 s, 10, 5,620,547,584, 5,621,458,261; 12 s, 14, 7,787,798,528, 7,787,343,189. R squared 1.000000. Acceptance, 60 s input with the new chooser: chunk 9 s, available 13,398,614,016 B, budget 6,699,307,008 B, predicted 6,162,929,493 B, measured 6,161,948,672 B, error -0.02 percent. With the calibrated model the injected 2 GiB and 4 GiB cases are below the floor and fail with the memory message; the 8 GiB and 16 GiB cases pick 5 s and 13 s. Gate: `pnpm verify` exit 0. Board 56 records, tasks unittest 9 passed, Vitest 1 passed, cargo test 150 passed and 2 ignored, pytest 14 passed. Evidence: `SCRATCH/evidence-05/`. Task 33 replaces the ONNX Runtime build with a pinned official release. The orchestrator re-runs the 60 s measurement on the merged tree after both tasks merge. Rebase onto master `102f78c` on 2026-10-11: the merged tree loads ONNX Runtime 1.28.3, and it re-measures the 60 s acceptance at -0.54 percent (chunk 19 s, predicted 11,641,145,813 B, measured 11,578,302,464 B) and the 600 s check at -3.89 percent (chunk 18 s, predicted 11,671,210,581 B, measured 11,217,575,936 B), and `pnpm verify` exits 0 with board 62 records and cargo test 191 passed and 3 ignored.

**Fix round 1 (review findings B1, B2, B3).** `choose_chunk` now takes the file shape (`FileShape { frames, channels }`) and reserves the engine's whole-file buffers before it picks a chunk. `file_reserve_bytes` is `(2 x channels + 2) x frames x 4` bytes: the engine part `(channels + 2) x frames x 4` (confirmed from `inference/mod.rs:362`, `:363`, `:411`, and `:467`) plus the decoded input `channels x frames x 4`. The chooser picks the largest chunk whose reserve plus predicted inference peak fits half the available memory. `ChunkBudget` now carries and displays the reserve, the inference peak, the total, the budget, and the available memory. `InferenceError::InsufficientMemory` now carries the required total, the budget (half the available memory), and the available memory, and states that the total exceeds the budget. Six new unit tests cover the reserve, a long mono file, a long stereo file, an exact budget match, and the one-hour six-channel rejection; the two ported reviewer tests are among them. Gate: `pnpm verify` exit 0. Board 56 records, tasks unittest 9 passed, Vitest 1 passed, cargo test 156 passed and 3 ignored, pytest 14 passed. Measurements (bytes and GB, decimal 1 GB = 1e9 bytes):

| Configuration | Chunk | Prediction model | Predicted peak | Measured peak | Error |
| --- | --- | --- | --- | --- | --- |
| 60 s stereo, arena on (the +96% run) | 17 s | task 01 one-channel line | 11,380,000,000 B (11.38 GB) | 22,319,132,672 B (22.32 GB) | +96.13% |
| 60 s stereo, arena on, diagnosis (a) | 17 s | task 01 one-channel line | 11,380,000,000 B (11.38 GB) | 22,318,972,928 B (22.32 GB) | +96.1% |
| 60 s stereo, arena off and patterns off, diagnosis (b) | 17 s | task 01 one-channel line | 11,380,000,000 B (11.38 GB) | 10,493,063,168 B (10.49 GB) | -7.8% |
| 60 s mono, arena on, diagnosis (c) | 17 s | task 01 one-channel line | 11,380,000,000 B (11.38 GB) | 22,299,820,032 B (22.30 GB) | +95.9% |
| 60 s stereo, arena off, fit point | 4 s | fitted engine line | 3,455,573,333 B (3.46 GB) | 3,456,028,672 B (3.46 GB) | -0.01% |
| 60 s stereo, arena off, fit point | 8 s | fitted engine line | 5,621,458,261 B (5.62 GB) | 5,620,547,584 B (5.62 GB) | +0.02% |
| 60 s stereo, arena off, fit point | 12 s | fitted engine line | 7,787,343,189 B (7.79 GB) | 7,787,798,528 B (7.79 GB) | -0.01% |
| 60 s stereo, arena off, acceptance round 1 | 9 s | fitted engine line | 6,162,929,493 B (6.16 GB) | 6,161,948,672 B (6.16 GB) | -0.02% |
| 60 s stereo, arena off, fix round 1 | 18 s | fitted engine line + file reserve | 11,099,674,581 B (11.10 GB) | 11,036,528,640 B (11.04 GB) | -0.57% |
| 600 s stereo, arena off, fix round 1 (the real check) | 16 s | fitted engine line + file reserve | 10,588,268,117 B (10.59 GB) | 10,171,662,336 B (10.17 GB) | -3.93% |

Real-time factor, 6 s parity input, stereo:

| Configuration | Elapsed | Real-time factor |
| --- | --- | --- |
| arena on | 14.342 s | 2.3903 |
| arena off | 15.174 s | 2.5289 (+5.8%) |

## Merge review

- Executors: `deepseek-v4.1-flash` (high) in CommandCode: a first session, a fresh session for fix round 1, and a fresh session for the rebase. Reviewers: two fresh Codex `gpt-6.1-sol` (medium) runs, adversarial.
- Review 1 on `8649843`: FAIL. B1: the chooser ignored the engine's whole-file buffers. B2: the below-floor error omitted the 50% budget. B3: `Outcome` had no measurement table. Fix round 1 (tests first at `8616759`): the chooser reserves `(2 x channels + 2) x frames x 4` bytes, the error names the total, the budget, and the available memory, and `Outcome` has the tables.
- Review 2 on `3e19ed4`: PASS. The reviewer confirmed the reserve, the long mono, long stereo, and six-channel cases, the error text, and the measurements (60 s -0.57%, 600 s -3.93%).
- Rebase: task 33 changed `load_onnx_backend` to take the runtime library path, so the reviewed commits did not apply to `master`. A fresh executor cherry-picked the eight commits onto `102f78c` as `task/05-rebased`. `load_onnx_backend_with_options(runtime_library, model, options)` calls `init_runtime` first, and `runtime_version(library)` reads `OrtGetApiBase` through `libloading` (already in the lock through `ort`). The orchestrator compared the branches with `git range-diff`: the commits map one to one, and the other differences are formatting. The orchestrator ruled that this adaptation needs no new Codex review.
- Re-measured on ONNX Runtime 1.28.3 on the rebased tree: 60 s -0.54% (chunk 19 s), 600 s -3.89% (chunk 18 s). Gate on `task/05-rebased`: `pnpm verify` exit 0, board 62, cargo test 191 passed and 3 ignored, pytest 19.
- Integration: the nine commits cherry-picked onto `master` `2fcf1ba` (task 34 merged, task 35 docs) without conflicts. The orchestrator ran the gate on the merged tree: `pnpm verify` exit 0, board 66, tasks unittest 9, Vitest 1, cargo test 193 passed and 3 ignored, pytest 19.
- Backlog filed: B-031 (timing medians and 60 s output parity), B-032 (overflow in the peak and reserve arithmetic), B-033 (stale line anchors and error-sign conventions in the records).

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

- [x] A restore of a 60 s fixture peaks within ±15% of the predicted value. The numbers are in `Outcome`.
- [x] The new tests failed first for the expected reason.
- [x] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core chunk_budget
pnpm verify
```
