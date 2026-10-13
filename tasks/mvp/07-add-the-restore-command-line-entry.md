# 07 — Add the restore command-line entry

**Status:** DONE
**Phase:** 1 — Restoration engine
**Depends on:** 04, 05, 06, 33
**SRS:** §6, §7, §12
**Stack:** §B
**Assigned to:** Agent
**Started:** 2026-10-13 — Base commit e429cd4, clean tree, `git lfs pull` in the worktree. Baseline gate at e429cd4 (recorded by the orchestrator): board 69 records, tasks unittest 9 passed, Vitest 1 passed, cargo test 193 passed and 3 ignored, pytest 19 passed. The branch rebased onto `cf899ea` (task 26 merged) before the gate.
**Outcome:** DONE 2026-10-13. `melomae-cli restore <in> <out> [--format flac|wav|opus] [--device auto|cpu] [--model <path>] [--onnxruntime <path>]` chains decode (03), chunk sizing (05), inference (04), and encode (06), and holds no pipeline logic of its own. `clap` 4.6.7 (derive; `MIT OR Apache-2.0`) parses the arguments and `ctrlc` 3.5.2 (`MIT/Apache-2.0`) installs the cancel handler. The Apollo model defaults to `apollo.onnx` beside the executable and the runtime to `onnxruntime.dll` (Windows) or `libonnxruntime.so` (Linux) beside it; a missing file exits 1 with a message that names the path and the flag. `auto` and `cpu` both run on the CPU; `gpu` is a clap usage error (exit 2) until task 08. Every error exits 1; a cancel exits 130 and writes no output file. The chunk is `choose_chunk_for_machine` (task 05), with 1 s of overlap and 1 s of pad. Progress is one stderr line per chunk and channel: `chunk n/m channel c/C elapsed S.Ss`.
    
    Parity (18 s input, WAV output, adaptive chunk 19 s): channel 0 max abs 3.611147e-4 and RMSE 2.271771e-5; channel 1 max abs 3.088117e-4 and RMSE 1.847152e-5, against the §12.1 limits 1e-3 and 1e-4. The task 04 golden was made at chunk 6 s, so this also confirms the padded chunk is effectively chunk-size invariant.
    
    Acceptance 1 (4-minute input, 40 copies of `input_wav.wav`): wall 740.8 s for 240 s stereo, end-to-end RTF 3.0868; chunk 19 s; available 24,403,832,832 B; predicted total 11,831,657,813 B; measured peak 11,736,076,288 B (child `PeakWorkingSetSize`, the counter task 05 read), error -0.81 percent. The Stack §K baseline is torch CPU RTF 2.70. The §12.2 bound is 1.2 x 2.70 = 3.24; this end-to-end RTF includes model load, decode, and encode, and it holds. Task 05 measured the engine alone at 2.53 on this machine. Evidence: `SCRATCH/evidence-07/four-minute.txt`.
    
    Acceptance 2 (stripped PATH): `PATH=C:\Windows\System32;C:\Windows`, the release binary with no `--model` and no `--onnxruntime` (both beside the executable), exit 0, output 2,116,844 B. Evidence: `SCRATCH/evidence-07/stripped-path.txt`.
    
    Ctrl-C is proven through the real binary, not a library flag: the test starts the child in its own process group and sends `CTRL_BREAK_EVENT` (Windows) or `SIGINT` (Linux) after the first chunk line, then asserts exit 130 and no output and no temporary file. `ctrlc` handles `CTRL_BREAK_EVENT` on Windows: its handler ignores the control-event code (`ctrlc-3.5.2/src/platform/windows/mod.rs:30`) and its docs name `CTRL_C_EVENT` or `CTRL_BREAK_EVENT` (`src/lib.rs:18`).
    
    Tests: 11 unit in the crate, 8 fast command-line tests, 3 model e2e tests (plus 1 `#[ignore]` 4-minute test), and 1 model cancel test. Red-first: `SCRATCH/evidence-07/red.txt`, exit 101, every test failed because the stub binary had no `restore` subcommand. Green: `SCRATCH/evidence-07/green.txt` and `SCRATCH/evidence-07/recheck.txt`, exit 0. Gate `pnpm verify` exit 0 (`SCRATCH/evidence-07/verify.txt`): board 72 records, tasks unittest 9 passed, Vitest 1 passed, cargo test 216 passed and 4 ignored, pytest 45 passed. The +23 passed and +1 ignored over the e429cd4 baseline are this task's tests. `python tools/cargo_deny.py check licenses` prints `licenses ok`; `python tools/generate_notices.py --check` is current; `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` are clean.
    
    Notes for later tasks: the CLI replaces an existing output, so the SRS §8.3 skip rule is task 17's; `encode` runs to completion with no cancel, so a cancel lands during inference only; the heavy CLI tests hold a process-wide mutex and each restore needs about 7 GB available because the chooser's floor is a 4 s chunk.

## Goal

Give agents and tests one end-to-end restore path that needs no GUI:

```
melomae-cli restore <in> <out> [--format flac|wav|opus] [--device auto|cpu]
```

## Scope

1. Chain decode (03), chunk sizing (05), inference (04), and encode (06).
2. Print one progress line per chunk.
3. Exit codes: 0 on success, 1 on error, 130 when Ctrl-C cancels the run.
4. On cancel, leave no output file.

## Out of scope

- GPU devices (task 08 adds `--device gpu`)
- Tags (task 15)

## Test-first plan

1. Write a CLI integration test that restores each task 03 fixture. Each output must decode.
2. For the 44.1 kHz WAV input, the output must match the task 04 golden within the §12.1 tolerances.

Run the tests. They fail because the command does not exist.

## Acceptance criteria

- [x] A 4-minute input (40 copies of `input_wav.wav`) restores on CPU. `Outcome` records the RTF and the peak memory against the Stack §K baseline.
- [x] One run passes with PATH stripped of conda, Python, and ffmpeg.
- [x] The new tests failed first for the expected reason.
- [x] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-cli
cargo run --release -p melomae-cli -- restore <fixture> <output> --format flac
pnpm verify
```
