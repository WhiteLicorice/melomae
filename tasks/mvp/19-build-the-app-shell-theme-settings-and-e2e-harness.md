# 19 — Build the app shell, theme, settings, and E2E harness

**Status:** TODO
**Phase:** 4 — Desktop UI
**Depends on:** 18
**SRS:** §2, §6, §7
**Stack:** §A, §I
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Give the app its navigation, theme, settings screen, and an end-to-end test harness that drives the real built app.

## Scope

1. A sidebar with Search, Library, Queue, Review, Local restore, and Settings.
2. Light and dark themes that follow the OS setting. Every text pair meets WCAG 2.2 AA contrast.
3. Full keyboard navigation with a visible focus ring.
4. Settings: library folder, output format (default FLAC), restore default (on), device (Auto), and keep sources (on).
5. An E2E harness with tauri-driver and WebdriverIO on Windows and Linux. Confirm current tauri-driver support at execution time.

## Out of scope

- The feature screens (tasks 20 to 25)

## Test-first plan

1. Vitest component tests: a settings change persists through the bridge.
2. One E2E test launches the built app, opens Settings, and changes the output format.

Run the tests. They fail because the shell does not exist.

## Acceptance criteria

- [ ] The E2E test passes against the built app, not the dev server.
- [ ] `Outcome` records a contrast check for both themes.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
pnpm test:e2e
pnpm verify
```
