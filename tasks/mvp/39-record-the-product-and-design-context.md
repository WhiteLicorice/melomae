# 39 — Record the product and design context

**Status:** DONE
**Phase:** 4 — Desktop UI
**Depends on:** —
**SRS:** §1, §3
**Stack:** §A
**Assigned to:** Agent
**Started:** 2026-11-18. Base `68c2491` on `task/39`, tree clean. No gate had run on this branch. The base has no `PRODUCT.md`, no `DESIGN.md`, and no design test.
**Outcome:** DONE 2026-11-18. `PRODUCT.md` follows the impeccable `init` schema (`impeccable:product-schema 1`) and omits `Stack`, because the repository already answers it. `DESIGN.md` follows the impeccable `document` format: YAML frontmatter with `colors`, `typography`, `rounded`, and `spacing`, then the eight canonical sections. The sidecar `.impeccable/design.json` is not written: the repository has no `.impeccable` directory and no panel reads it. The new `control-edge` token is `#857D71` light and `#7C746A` dark. It reaches 3.61:1 on the canvas and 3.87:1 on the surface in light, and 3.93:1 and 3.61:1 in dark. `border` stays for dividers. The `focus` ring reaches 7.69:1 to 11.79:1 in both themes. The lowest text pair is `warn` on the light canvas at 5.26:1. `DESIGN.md` names the system UI and monospace stacks, the 1100 x 720 default and 800 x 600 minimum window, the 216 px rail, the 1280 px content ceiling, the 4 px spacing rhythm, the 80 ms and 140 ms motion steps, and the reduced-motion rule. `tests/design.test.ts` reads the frontmatter and the 19-row table, computes WCAG contrast, pins two known ratios (21:1 and 4.48:1), and fails any pair below its minimum. The test sits outside `src`, because the task keeps `src` untouched; `vitest.config.ts` and `tsconfig.json` now include `tests`. `tasks/README.md` step 2 lists both documents. Red-first, three runs: `.claygo/mt-evidence/reports/39-red.txt` failed on the missing `../DESIGN.md?raw`; `.claygo/mt-evidence/reports/39-red-border.txt` failed at 1.33:1 and 1.53:1 with the proposal's `border` as the control edge; `.claygo/mt-evidence/reports/39-red-text.txt` failed at 2.67:1 with `ink-muted` degraded to `#9A948C`. Gate `pnpm verify` exit 0 (`.claygo/mt-evidence/reports/39-verify.txt`): board 123 records valid; tasks unittest 12 passed; typecheck, lint, and format clean; Vitest 2 files and 6 tests passed; `cargo test --workspace` 276 passed, 0 failed, 4 ignored; pytest 45 passed in 606 s with one pre-existing FutureWarning; notices and licenses current. Clippy `--workspace --all-targets -- -D warnings` passed separately, exit 0. Not verified: no stylesheet was rendered in a webview and no pixel was measured; the ratios are computed from sRGB values. The claim most likely to be wrong is that no width breakpoint is needed: at 800 px the six table columns plus the 216 px rail leave 584 px, and no long file name was tested against it. Task 19 owns the resize matrix.

## Goal

Give tasks 19 to 25 one recorded source for the users, the tone, and the design tokens, so that each screen does not invent its own.

## Evidence

- The repository root has no `PRODUCT.md` and no `DESIGN.md`. The impeccable audit of 2026-11-11 at `master` `0220b19` listed the root and found neither.
- `docs/melomae-srs.md` defines behavior. It has no section for tone, brand, or visual rules.
- `src/App.css:8-22` hard-codes the colors and names the `Inter` typeface. No `@font-face` rule and no font file exists, so Inter cannot load.
- `tasks/ux/interface-proposal.md` sections 1 and 2 hold a proposed character and a proposed token table. Its MVP ruling of 2026-11-11 accepts them with changes.
- The proposal's `border` token gives 1.33:1 to 1.53:1 on its backgrounds. A control edge needs 3:1.

## Scope

1. Read the MVP ruling in `tasks/ux/interface-proposal.md`. It wins over sections 1 and 2. Start from the token table in section 2.
2. Write `PRODUCT.md`: the users (SRS §3.1), the tone, the brand commitments, and at least one thing that the interface must not look like.
3. Write `DESIGN.md`: tokens for color, type, spacing, radius, focus, and motion. Each color token has a light value and a dark value.
4. Add a control-edge token that reaches 3:1 on each background. Keep `border` for dividers.
5. Name the system font stack for the UI and for monospace text. Do not bundle a font, and do not load one from the network (SRS §3.4).
6. State the window sizes, the breakpoints, and the reduced-motion policy in `DESIGN.md`.
7. Add both files to step 2 of the executor protocol in `tasks/README.md`.

## Out of scope

- Any change to `src` (task 19 applies the tokens)

## Test-first plan

1. A test reads the color tokens from `DESIGN.md`, or from the token file that it names, and computes the contrast of each text pair in both themes. It fails below 4.5:1. It fails below 3:1 for the control edge and the focus ring.

Run the test. It fails because the token source does not exist.

## Acceptance criteria

- [x] `PRODUCT.md` and `DESIGN.md` exist, and the executor protocol lists them.
- [x] Every text pair computes to 4.5:1 or more in both themes.
- [x] The control edge and the focus ring compute to 3:1 or more.
- [x] The new test failed first for the expected reason.
- [x] `pnpm verify` passes.

## Verify

```powershell
pnpm test
pnpm verify
```
