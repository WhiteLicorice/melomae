# 19 — Build the app shell, theme, settings, and E2E harness

**Status:** TODO
**Phase:** 4 — Desktop UI
**Depends on:** 18, 39
**SRS:** §2, §6, §7
**Stack:** §A, §I
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Give the app its navigation, theme, settings screen, and an end-to-end test harness that drives the real built app.

## Evidence

The impeccable audit of 2026-11-11 at `master` `0220b19` read the starter stylesheet that this task replaces:

- `src/App.css:89-92` sets `outline: none` on every input and button. The file has no `:focus` or `:focus-visible` rule.
- `src/App.css:51`: the link color `#646cff` computes to 3.79:1 on the light background `#f6f6f6` and to 3.27:1 on the dark background `#2f2f2f`. WCAG 2.2 AA needs 4.5:1. The dark block at `:98-116` changes only the hover color.
- `src/App.css:66` and `:74`: a control has a transparent border and a weak shadow. Its edge is below 3:1.
- The file has no `color-scheme` property and no `prefers-reduced-motion` block.
- `src-tauri/tauri.conf.json:13-18`: the window is 800 x 600 with no minimum size and no background color.
- `src/main.tsx:5-9`: the root render has no error boundary. A render error leaves a blank window.

The ratios are computed from the declared colors. Nobody measured a rendered pixel.

`tasks/ux/interface-proposal.md` sections 2, 3.6, and 8 propose the frame, the Settings sections, and the keyboard contract. It is a proposal until the owner accepts it.

## Scope

1. A sidebar with Restore, Queue, History, and Settings. The sidebar is a named `nav` landmark, the active item has `aria-current="page"`, and a skip link to the main content is the first focusable element.
2. Light and dark themes that follow the OS setting. Every text pair meets WCAG 2.2 AA contrast.
3. Full keyboard navigation with a visible focus ring.
4. Settings: default output folder, output format (default FLAC), restore default (on), device (Auto), and volume matching (on).
5. An E2E harness with tauri-driver and WebdriverIO on Windows and Linux. Confirm current tauri-driver support at execution time.
6. Replace the starter stylesheet with the tokens from `DESIGN.md` (task 39). Remove the `outline: none` rule. Every control gets one `:focus-visible` ring.
7. Every control edge, the focus ring, and the progress track reach 3:1 against the adjacent color. A status never uses color alone. It has a text label.
8. Set `color-scheme: light dark` on `:root`, and set the window background color. Confirm at execution time that Linux WebKitGTK reports the OS theme. This behavior is inferred and not confirmed.
9. Set `minWidth` and `minHeight` for the window. The shell works at the minimum size and at 200% zoom.
10. Follow the reduced-motion policy in `DESIGN.md`: no movement, and state changes still show.
11. Put an error boundary around the app: a plain message, a Reload action, and a detail that the user can copy.

## Out of scope

- The feature screens (tasks 20 to 25)

## Test-first plan

1. Vitest component tests: a settings change persists through the bridge.
2. One E2E test launches the built app, opens Settings, and changes the output format.
3. A test reads the stylesheet and fails when an `outline: none` rule exists with no `:focus-visible` rule. jsdom does not apply the stylesheet, so the E2E test also reads the computed outline of a focused control in the built app.
4. A component test checks the `nav` landmark, `aria-current`, and that the skip link takes the first Tab.
5. A component test throws in a child and sees the error boundary message and the Reload action.
6. The E2E test runs at the minimum window size.

Run the tests. They fail because the shell does not exist.

## Acceptance criteria

- [ ] The E2E test passes against the built app, not the dev server.
- [ ] `Outcome` records a contrast check for both themes.
- [ ] A focused control shows a ring in the built app, in both themes.
- [ ] The window cannot go below the minimum size, and no text clips there at 200% zoom.
- [ ] `Outcome` records the Linux theme check.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
pnpm test:e2e
pnpm verify
```
