# 07 — Add the restore command-line entry

**Status:** DONE
**Phase:** 1 — Restoration engine
**Depends on:** 04, 05, 06, 33
**SRS:** §6, §7, §12
**Stack:** §B
**Assigned to:** Agent
**Started:** 2026-10-13 — Base commit e429cd4, clean tree, `git lfs pull` in the worktree. Baseline gate at e429cd4 (recorded by the orchestrator): board 69 records, tasks unittest 9 passed, Vitest 1 passed, cargo test 193 passed and 3 ignored, pytest 19 passed. The branch rebased onto `cf899ea` (task 26 merged) before the gate.
**Outcome:** DONE 2026-10-13, fix round 1. `melomae-cli restore <in> <out> [--format flac|wav|opus] [--device auto|cpu] [--model <path>] [--onnxruntime <path>]` chains decode (03), chunk sizing (05), inference (04), and encode (06), and holds no pipeline logic of its own. `clap` 4.6.7 (derive; `MIT OR Apache-2.0`) parses the arguments and `ctrlc` 3.5.2 (`MIT/Apache-2.0`) installs the cancel handler before any long work. The Apollo model defaults to `apollo.onnx` beside the executable and the runtime to `onnxruntime.dll` (Windows) or `libonnxruntime.so` (Linux) beside it; a missing file exits 1 with a message that names the path and the flag. `auto` and `cpu` both run on the CPU; `gpu` is a clap usage error (exit 2) until task 08. The output preflight runs before decode: the CLI refuses an output that resolves to the input (file identity through `same-file` 1.0.6, `Unlicense/MIT`, plus a case-insensitive canonical parent-and-name comparison on Windows), an existing output, and a missing output directory, each with a message that names it; the CLI never replaces a file and never creates the folder. The chunk is `choose_chunk_for_machine` (task 05), with 1 s of overlap and 1 s of pad. Progress is one stderr line per temporal chunk, `chunk n/m elapsed S.Ss`; the channels run one after the other, so the chunk is reported when its last channel starts. An integer-format encode prints `melomae-cli: clamped N samples` on stderr when N is not zero (SRS §7.4). A cancel exits 130 and leaves no output and no temporary file: the handler is installed first, the cancel flag is checked between stages, and `melomae_core::encode::encode_with_cancel` checks it at every write and once more before the final rename, removing its temporary file and leaving the target untouched on cancel. `encode` keeps its signature and calls the new function with a flag that is never set.

Parity (18 s input from three copies of `input_wav.wav`, WAV float output, the adaptive chunk the chooser picked on the gate machine): channel 0 max abs 3.611147e-4 and RMSE 2.271771e-5; channel 1 max abs 3.088117e-4 and RMSE 1.847152e-5, against the §12.1 limits 1e-3 and 1e-4. This confirms parity for that fixture across those settings; it does not establish chunk-size invariance generally.

Acceptance 1 (4-minute input, 40 copies of `input_wav.wav`): wall 740.8 s for 240 s stereo, end-to-end RTF 3.0868; chunk 19 s; available 24,403,832,832 B; predicted total 11,831,657,813 B; measured peak 11,736,076,288 B (child `PeakWorkingSetSize`, the counter task 05 read), error -0.81 percent. The Stack §K baseline is torch CPU RTF 2.70; the §12.2 bound 1.2 x 2.70 = 3.24 holds. This end-to-end RTF includes model load, decode, and encode. Task 05 measured the engine alone at 2.53 on this machine. Evidence: `SCRATCH/evidence-07/four-minute.txt`.

Acceptance 2 (stripped PATH): `PATH=C:\Windows\System32;C:\Windows`, the release binary with no `--model` and no `--onnxruntime` (both beside the executable), exit 0, output 2,116,844 B. Evidence: `SCRATCH/evidence-07/stripped-path.txt`.

Ctrl-C is proven through the real binary, not a library flag: three tests start the child in its own process group and send `CTRL_BREAK_EVENT` (Windows) or `SIGINT` (Linux) — after the first progress line, after the start line before any chunk line, and once the encoder's temporary file exists — then assert exit 130 and no output and no temporary file. `ctrlc` handles `CTRL_BREAK_EVENT` on Windows: its handler ignores the control-event code (`ctrlc-3.5.2/src/platform/windows/mod.rs:30`) and its docs name `CTRL_C_EVENT` or `CTRL_BREAK_EVENT` (`src/lib.rs:18`).

Tests: 11 unit in the crate, 11 fast command-line tests, 5 model e2e tests (plus 1 `#[ignore]` 4-minute test), and 3 model cancel tests. Red-first (fix round 1): `SCRATCH/evidence-07/red-round1.txt`, exit 101. Green: `SCRATCH/evidence-07/green-round1.txt`. Gate `pnpm verify` exit 0 (`SCRATCH/evidence-07/verify-round1.txt`): board 72 records, tasks unittest 9 passed, Vitest 1 passed, cargo test 225 passed and 4 ignored, pytest 45 passed. `python tools/cargo_deny.py check licenses` prints `licenses ok`; `python tools/generate_notices.py --check` is current; `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` are clean.

Notes for later tasks: the CLI refuses to replace an existing output, so the app's skip-or-replace policy (SRS §8) is task 17's; `encode` still replaces its target when the caller does not preflight; the heavy CLI tests hold a process-wide mutex and each restore needs about 7 GB available because the chooser's floor is a 4 s chunk.

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
