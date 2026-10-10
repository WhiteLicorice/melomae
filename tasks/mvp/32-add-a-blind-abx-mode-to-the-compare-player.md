# 32 — Add a blind ABX mode to the compare player

**Status:** TODO
**Phase:** 4 — Desktop UI
**Depends on:** 23
**SRS:** §10
**Stack:** §H
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Let the user test whether they can hear the restoration, without knowing which version plays.

## Scope

1. Add a blind mode to the task 23 player per SRS §10.6 and §10.7.
2. Hide the labels and the gain offset during the trials.
3. Choose X with a random source that the test can seed.
4. After the user ends the session, show the correct count, the trial count, and the one-sided binomial p-value with p = 0.5.
5. The A, B, and X switch keeps the playback position and takes 50 ms or less (SRS §12.5).

## Out of scope

- Saving ABX results to the history

## Test-first plan

1. With a fixed seed, the X sequence is reproducible.
2. The p-value function matches known values: 9 of 10 gives about 0.0107, and 5 of 10 gives about 0.623.
3. Volume matching stays on in blind mode, and the toggle is disabled.
4. A component test runs three trials and checks the result screen.

Run the tests. They fail because the mode does not exist.

## Acceptance criteria

- [ ] `Outcome` records the measured switch time in blind mode.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core playback
pnpm test
pnpm verify
```
