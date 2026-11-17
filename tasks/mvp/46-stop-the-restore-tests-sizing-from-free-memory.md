# 46 — Stop the restore tests sizing from free memory

**Status:** TODO
**Phase:** 1 — Restoration engine
**Depends on:** 36
**SRS:** §6, §12
**Stack:** §C
**Assigned to:** Agent
**Started:** —
**Outcome:** —

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
- Inferred, not confirmed: at a fixed 6 s chunk (the core parity setting, `crates/melomae-core/tests/inference_parity.rs:44`) each CLI needs well under 1 GB, so the tests fit in parallel. A measured peak per test confirms or rejects this.

## Scope

1. Give every CLI restore test (`restore_e2e`, `restore_signals`, `restore_args` where it runs a restore) fixed chunk settings, through the chunk flag that task 36 adds or an equivalent CLI option. No test may depend on free memory.
2. Keep at least one test that covers the free-memory chooser on purpose, with a budget the test sets, not the machine's.
3. Measure the peak memory of one restore test and the wall time of each restore binary, before and after.
4. Remove the `RUST_TEST_THREADS=1` stopgap from the orchestration notes when the gate passes without it.

## Out of scope

- A change to how the shipped app sizes chunks (task 05 owns the chooser).

## Test-first plan

1. With a low memory budget forced on the test process, or with the restore tests in parallel on a loaded machine, a restore test fails now with the budget error or "bad allocation".
2. After the change, the same run passes, and the tests run in parallel.

## Acceptance criteria

- [ ] No CLI restore test reads free memory, except the one test that covers the chooser with its own budget.
- [ ] `cargo test -p melomae-cli` passes with the tests in parallel, with no `RUST_TEST_THREADS` setting.
- [ ] The record states the peak memory per restore test and the wall time of each restore binary, before and after.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.
