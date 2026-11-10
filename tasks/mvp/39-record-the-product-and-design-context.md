# 39 — Record the product and design context

**Status:** TODO
**Phase:** 4 — Desktop UI
**Depends on:** —
**SRS:** §1, §3
**Stack:** §A
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Give tasks 19 to 25 one recorded source for the users, the tone, and the design tokens, so that each screen does not invent its own.

## Evidence

- The repository root has no `PRODUCT.md` and no `DESIGN.md`. The impeccable audit of 2026-11-11 at `master` `0220b19` listed the root and found neither.
- `docs/melomae-srs.md` defines behavior. It has no section for tone, brand, or visual rules.
- `src/App.css:8-22` hard-codes the colors and names the `Inter` typeface. No `@font-face` rule and no font file exists, so Inter cannot load.
- `tasks/ux/interface-proposal.md` sections 1 and 2 hold a proposed character and a proposed token table. The owner has not accepted them.
- The proposal's `border` token gives 1.33:1 to 1.53:1 on its backgrounds. A control edge needs 3:1.

## Scope

1. Read the `Owner decision` line in `tasks/ux/interface-proposal.md`. If the owner has not accepted or amended sections 1 and 2, stop and ask through the human-input protocol.
2. Write `PRODUCT.md`: the users (SRS §3.1), the tone, the brand commitments, and at least one thing that the interface must not look like.
3. Write `DESIGN.md`: tokens for color, type, spacing, radius, focus, and motion. Each color token has a light value and a dark value.
4. Add a control-edge token that reaches 3:1 on each background. Keep `border` for dividers.
5. Choose the typeface. Bundle its woff2 files and list them in the notices, or name the system font stack. Do not load a font from the network (SRS §3.4).
6. State the window sizes, the breakpoints, and the reduced-motion policy in `DESIGN.md`.
7. Add both files to step 2 of the executor protocol in `tasks/README.md`.

## Out of scope

- Any change to `src` (task 19 applies the tokens)

## Test-first plan

1. A test reads the color tokens from `DESIGN.md`, or from the token file that it names, and computes the contrast of each text pair in both themes. It fails below 4.5:1. It fails below 3:1 for the control edge and the focus ring.

Run the test. It fails because the token source does not exist.

## Acceptance criteria

- [ ] `PRODUCT.md` and `DESIGN.md` exist, and the executor protocol lists them.
- [ ] Every text pair computes to 4.5:1 or more in both themes.
- [ ] The control edge and the focus ring compute to 3:1 or more.
- [ ] The new test failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
pnpm test
pnpm verify
```
