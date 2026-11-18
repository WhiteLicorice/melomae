# 54 — Measure the GPU speed in a release build

**Status:** TODO
**Phase:** 1 — Restoration engine
**Depends on:** 08
**SRS:** §6
**Stack:** §C
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Record the GPU real-time factor and the GPU parity numbers from a release build, each with its log.

## Evidence

- The author ruled on 2026-10-14 that the release-build measurement is MVP on Windows. Task 08 measured only debug builds.
- The task 08 review (findings 4 and 8) found that the parity numbers in the task 08 Outcome come only from a log built in the withdrawn shared Cargo folder, and that RTF 0.5785 has no log. The only valid CLI run shows RTF 0.883 on a 6 s fixture (`08-cli-device.txt`).

## Scope

1. Run `cargo test -p melomae-core --test inference_gpu gpu_restore_matches -- --nocapture` in its own target folder, and save the printed parity lines.
2. Run the release CLI with `--device gpu` on the 6 s fixture and on a longer one, and save each log.
3. Correct the task 08 Outcome to quote these numbers with their logs.

## Acceptance criteria

- [ ] Each number in the task 08 Outcome cites a log made in its own target folder.
- [ ] `pnpm verify` passes.
