# 07 — Add the restore command-line entry

**Status:** TODO
**Phase:** 1 — Restoration engine
**Depends on:** 04, 05, 06
**SRS:** §6, §7, §12
**Stack:** §B
**Assigned to:** Agent
**Started:** —
**Outcome:** —

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

- [ ] A 4-minute input (40 copies of `input_wav.wav`) restores on CPU. `Outcome` records the RTF and the peak memory against the Stack §K baseline.
- [ ] One run passes with PATH stripped of conda, Python, and ffmpeg.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-cli
cargo run --release -p melomae-cli -- restore <fixture> <output> --format flac
pnpm verify
```
