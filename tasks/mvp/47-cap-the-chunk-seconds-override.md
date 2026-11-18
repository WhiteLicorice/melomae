# 47 — Cap the chunk-seconds override

**Status:** TODO
**Phase:** 1 — Restoration engine
**Depends on:** 36
**SRS:** §6.4, §12.3
**Stack:** §C
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Keep the user-facing `--chunk-seconds` flag inside the chunk limits and the half-RAM budget, and cover it in the help contract test.

## Evidence

- The task 36 review (2026-11-18) found that `resolve_chunking` (`crates/melomae-cli/src/restore.rs:92-96`) checks that the value is finite, positive, and obeys the overlap rule, but has no upper bound. `restore.rs:182` then uses it as given.
- So `--chunk-seconds 120` passes `MAX_CHUNK_SECONDS = 30` (`crates/melomae-core/src/chunk_budget.rs:33`) and the §6.4 and §12.3 half-RAM budget. The review computed `predicted_peak_bytes(122)` at about 66 GB.
- The flag shows in `--help`, but the help contract test (`crates/melomae-cli/tests/restore_args.rs:43`) doesn't list it.

## Scope

1. Reject an override outside `MIN_CHUNK_SECONDS..=MAX_CHUNK_SECONDS` with a clear message that names the range.
2. Add `--chunk-seconds` to the help contract test's option list.

## Out of scope

- Whether a fixed chunk skips the free-memory chooser. Task 46 owns that.

## Test-first plan

1. `melomae-cli restore --chunk-seconds 120 …` starts a run now. A test that expects a usage error fails.
2. The help contract test fails until it lists the flag, or passes and then catches its removal.

## Acceptance criteria

- [ ] An override below the minimum or above the maximum exits with a usage error that names the range.
- [ ] Values at both bounds are accepted.
- [ ] The help contract test lists `--chunk-seconds`.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.
