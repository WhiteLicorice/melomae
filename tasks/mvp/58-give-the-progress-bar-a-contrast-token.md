# 58 — Give the progress bar a contrast token

**Status:** TODO
**Phase:** 4 — Desktop UI
**Depends on:** 39
**SRS:** §1, §3
**Stack:** §A
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Make `PRODUCT.md` and `DESIGN.md` agree on the progress bar's 3:1 contrast.

## Evidence

- The task 39 review (finding 3): `PRODUCT.md:124` says "A control edge, a focus ring, and a progress bar reach 3:1." `DESIGN.md` has no progress-bar token, component rule, or contrast row. The claim comes from `tasks/ux/interface-proposal.md:343`.

## Scope

1. Add a progress-bar fill token to `DESIGN.md`, with a contrast row on `canvas` and on `surface` at a minimum of 3 in both themes.
2. The contrast test checks the new rows.

## Acceptance criteria

- [ ] `DESIGN.md` defines the progress-bar fill, and its rows reach 3:1 in both themes.
- [ ] The contrast test covers the new rows.
- [ ] `pnpm verify` passes.
