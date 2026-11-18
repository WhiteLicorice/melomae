# 46 — Stop the restore tests sizing from free memory

**Status:** DONE
**Phase:** 1 — Restoration engine
**Depends on:** 36
**SRS:** §6, §12
**Stack:** §C
**Assigned to:** Agent
**Started:** 2026-11-18. Base `b5ec0c4` on branch `task/46` from `master`, clean tree, target `C:\mt\t46`. No gate ran at the base. The last recorded gate, at `fd0606d`, failed twice from free memory (see Evidence). `.cache` copied from `C:\mt\w36`.
**Outcome:** DONE. `--chunk-seconds` now resolves inside the per-device `settings_for` (task 08's shape), so neither chooser runs: `chunk_settings` (`crates/melomae-cli/src/restore.rs:260`) calls `chunk_choice` (`:245`), which returns `ChunkChoice::Fixed` (`:197`) before the chooser for the CPU and for the GPU. `ChunkChoice::describe` (`:219`) takes the device and prints the used chunk's file reserve and the device's own predicted peak (`predicted_gpu_peak_bytes` on the GPU) with `fixed by --chunk-seconds`, and `settings_for` (`:355`) prints that line and runs `choice.seconds()`. Red first in `46-red.txt` at base `b5ec0c4`: with 0.22 GB free, `e2e_restore_wav_matches_golden` at `--chunk-seconds 6` failed inside `choose_chunk_for_machine` with "needs at least 3474624533 bytes, more than the 278769664-byte budget" (exit 101), and `a_fixed_chunk_never_calls_the_free_memory_chooser` failed with "the chooser must not run when --chunk-seconds is given". Round 2 red in `46-red-gpu.txt` at the rebased resolution: `a_fixed_gpu_chunk_never_calls_the_gpu_chooser` failed with "the GPU chooser must not run when --chunk-seconds is given" (exit 101), then passed. Every restore test now passes a fixed chunk - `restore_e2e` 6 s (`restore_e2e.rs:39`, golden keeps its 6 s and 17 s cases at `:291`, progress input 18 s at `:338`, the ignored four-minute test at 6 s) and `restore_signals` 4 s (`restore_signals.rs:33`, used at `:166` and `:248`) - and the one chooser test passes its own 8 GiB budget (`restore.rs:771`). Measured peak working set of one CLI on the 18 s input: 4 s 3.42 GB, 6 s 4.50 GB, 17 s 9.23 GB; so each `restore_signals` test peaks at 3.42 GB, the three 6 s `restore_e2e` tests at 4.50 GB, and the golden test's 17 s case at 9.23 GB, where before the chooser picked 19 s here (predicted 11.58 GB) and the chunk moved with free memory. Wall time per binary, before -> after: unit 0.00 -> 0.00 s, `restore_args` 0.17 -> 0.17 s, `restore_e2e` 465.63 -> 581.37 s (the fixed 6 s chunk runs three passes over the 18 s input instead of one), `restore_signals` 5.69 s (1 passed, 2 failed, `C:\mt\reports\44-verify-new.txt:104`) -> 185.26 s for three in parallel. `cargo test -p melomae-cli` with no `RUST_TEST_THREADS`: 31 passed, 1 ignored; the run's peak is the largest single CLI, 9.23 GB on the golden 17 s case (`C:\mt\reports\46-instruments\sample-melomae-cli.txt`). That sampler's 9.22 GB summed figure is not a simultaneous peak, because it samples current `WorkingSet64` while the single peak is `PeakWorkingSet64`, so the record does not use it. All-targets Clippy and `cargo fmt --check` clean (`46-clippy.txt`). Gate `pnpm verify` with no `RUST_TEST_THREADS`, exit 0 (`46-verify.txt`): board 122 records; unittest 12 passed; Vitest 1 passed; cargo 274 passed, 0 failed, 4 ignored across 36 binaries; pytest 45 passed, 1 pre-existing FutureWarning. Not verified: a gate on a machine with less free memory than this one (about 23 GB); the tests no longer read free memory, but four CLIs at 4.5 to 9.2 GB each still need the RAM to be there.

## Goal

Make every CLI restore test run at fixed, small chunk settings, so the tests in one binary can run in parallel without exhausting memory, and the gate gets faster and deterministic.

The owner ranked this task high on 2026-11-18 because the gate is the bottleneck for every other task.

## Evidence

- Each restore test starts `melomae-cli`, and the CLI sizes its chunk budget as half of the free memory when it starts. The tests in one binary run in parallel, so each CLI reads the same free memory and claims half of it.
- On 2026-11-18 the gate on `master` `fd0606d` failed twice with exit 101. The 4 `restore_e2e` tests (`crates/melomae-cli/tests/restore_e2e.rs`) failed at `:174` with "the restore needs at least 3456278933 bytes, more than the 1988038656-byte budget, which is half of the 3976077312 bytes available".
- The same day, the task 44 gate failed 2 of the 3 `restore_signals` tests. `e2e_restore_encode_cancel_exits_130_and_leaves_no_output` (`crates/melomae-cli/tests/restore_signals.rs:370`) logged "chunk 10s: … predicted inference peak 6704400725 bytes, … budget 7238293504 bytes, available 14476587008 bytes", then ONNX Runtime failed with "bad allocation" in `node_conv1d_87`. `e2e_restore_cancel_exits_130_and_leaves_no_output` failed at `:274` with no chunk progress line. Three parallel CLIs asked for about 20 GB together.
- One `melomae-cli` from these tests took 6.8 GB of private memory on the same day.
- The `restore_e2e` binary took 465.63 s with its 4 tests in parallel.
- The orchestrator set `RUST_TEST_THREADS=1` for the gate as a stopgap. It stops the failures but runs the restore tests one after another.
- The task 36 review (2026-11-18) found that `--chunk-seconds` doesn't stop the free-memory check. `choose_chunk_for_machine(shape)?` at `crates/melomae-cli/src/restore.rs:179` runs first and fails the run when it rejects. Only `:182` replaces its result with the flag's value. The `chunk budget:` line at `:192` then describes the chooser's chunk, not the chunk the run uses. So a test with a fixed chunk still fails on a machine with less than about 6.9 GB free.
- Inferred, not confirmed: at a fixed 6 s chunk (the core parity setting, `crates/melomae-core/tests/inference_parity.rs:44`) each CLI needs well under 1 GB, so the tests fit in parallel. A measured peak per test confirms or rejects this.

## Scope

1. When `--chunk-seconds` is given, skip `choose_chunk_for_machine` and print the predicted peak of the chunk the run uses (`restore.rs:178-192`).
2. Give every CLI restore test (`restore_e2e`, `restore_signals`, `restore_args` where it runs a restore) fixed chunk settings, through the chunk flag that task 36 adds or an equivalent CLI option. No test may depend on free memory.
3. Keep at least one test that covers the free-memory chooser on purpose, with a budget the test sets, not the machine's.
4. Measure the peak memory of one restore test and the wall time of each restore binary, before and after.
5. Remove the `RUST_TEST_THREADS=1` stopgap from the orchestration notes when the gate passes without it.

## Out of scope

- A change to how the shipped app sizes chunks (task 05 owns the chooser).

## Test-first plan

1. With a low memory budget forced on the test process, or with the restore tests in parallel on a loaded machine, a restore test fails now with the budget error or "bad allocation".
2. After the change, the same run passes, and the tests run in parallel.

## Acceptance criteria

- [x] A run with `--chunk-seconds` never calls the free-memory chooser, and its budget line describes the chunk it uses.
- [x] No CLI restore test reads free memory, except the one test that covers the chooser with its own budget.
- [x] `cargo test -p melomae-cli` passes with the tests in parallel, with no `RUST_TEST_THREADS` setting.
- [x] The record states the peak memory per restore test and the wall time of each restore binary, before and after.
- [x] The new tests failed first for the expected reason.
- [x] `pnpm verify` passes.
