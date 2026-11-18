# 53 — Size the GPU chunk from free video memory

**Status:** TODO
**Phase:** 1 — Restoration engine
**Depends on:** 08
**SRS:** §6
**Stack:** §C
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Read the free video memory and size the GPU chunk from it, in place of the fixed 6 s chunk.

## Evidence

- The author ruled on 2026-10-14 that the video-memory query is MVP on Windows. It was left out of task 08.
- `vram_bytes()` returns `None` (`crates/melomae-cli/src/restore.rs:159-160`), so `choose_gpu_chunk(Some(..))` never runs and the fixed 6 s chunk ships.
- The task 08 review (finding 5) found that the GPU peak constants (`crates/melomae-core/src/inference/chunk_budget.rs:155-165`) cite `SCRATCH/evidence-08/`, which does not exist.

## Scope

1. Query the free video memory of the CUDA device on Windows.
2. Pass it to `choose_gpu_chunk`. Keep the fixed chunk when the query fails.
3. Measure the GPU peak again, save the log in the task record, and cite it beside the constants.

## Acceptance criteria

- [ ] The CLI prints the queried budget on a CUDA machine.
- [ ] The constants cite a log that exists.
- [ ] `pnpm verify` passes.
