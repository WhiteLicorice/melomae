# 59 — Give the engine lock its real reason

**Status:** TODO
**Phase:** 1 — Restoration engine
**Depends on:** 46
**SRS:** §6, §12
**Stack:** §C
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Make the comment on the restore test engine lock state why the lock exists.

## Evidence

- The task 46 review (2026-11-18, `.claygo/mt-evidence/reports/46-review.md` non-blocking 3): `crates/melomae-cli/tests/restore_e2e.rs:78` says parallel threads would "make the chunk chooser reject a chunk that fits on an idle machine". Every restore run in that binary now passes `--chunk-seconds`, so no chooser runs.
- The lock still keeps the 17 s golden case, about 9.9 GB, alone in memory.

## Scope

1. Keep the lock. Reword its comment to the memory headroom of the golden case.

## Acceptance criteria

- [ ] The comment names the memory headroom, not the chooser.
- [ ] `cargo test -p melomae-cli` passes.
