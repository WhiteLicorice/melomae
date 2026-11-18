# 48 — Guard the last-chunk clamp with tests

**Status:** TODO
**Phase:** 1 — Restoration engine
**Depends on:** 36
**SRS:** §6
**Stack:** §C
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Make the regression tests for task 36's last-chunk change cover every path that the change touches, so a later edit can't break it unnoticed.

## Evidence

- The task 36 review (2026-11-18) found that only the 17 s case of the four matrix lengths (`crates/melomae-core/tests/inference_parity.rs:51`) reaches the changed code. 18 s and 19 s take `run_full_file`, and at 16 s the old zero fill had no effect. Task 36's red log (`36-red.txt:128-135`) shows 16, 18, and 19 s passing before the fix.
- The committed unit test's two chunks both see the whole 1000-sample file. Nothing covers the clamp on a chunk before the last, or the `pad = 0` case (`crates/melomae-core/src/inference/mod.rs:328`, `:411`).
- The reviewer probed both cases by hand and the code is correct. The gap is in the guard, not the fix.

## Scope

1. Add two fake-model unit cases: the clamp on a chunk before the last, and `pad = 0`. They need no model.
2. Replace one of the 18 s and 19 s matrix lengths with 16.5 s, which also reaches the changed path.

## Out of scope

- Any change to the inference code.

## Test-first plan

1. Revert task 36's clamp in a scratch copy. Each new case fails there for the reason it names.
2. With the clamp in place, they pass.

## Acceptance criteria

- [ ] A fake-model test covers the clamp on a chunk before the last.
- [ ] A fake-model test covers `pad = 0`.
- [ ] The parity matrix has two lengths that reach the changed path.
- [ ] Each new test fails with the clamp reverted.
- [ ] `pnpm verify` passes.
