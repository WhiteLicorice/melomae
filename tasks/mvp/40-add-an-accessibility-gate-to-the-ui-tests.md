# 40 — Add an accessibility gate to the UI tests

**Status:** TODO
**Phase:** 4 — Desktop UI
**Depends on:** 19
**SRS:** §3
**Stack:** §A, §I
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Make the gate fail when a screen ships an unlabeled control, a missing landmark, or a missing role.

## Evidence

- `src/App.test.tsx:6-13` is the only UI test. It queries one heading.
- `package.json:7-16` has no accessibility script.
- Task 19 adds the E2E harness and records a manual contrast check. A contrast record does not find a missing label or role.
- The impeccable audit of 2026-11-11 at `master` `0220b19` named this gap.

## Scope

1. Add an axe scan to the Vitest component tests. The candidate is `vitest-axe`. Confirm its support for the installed Vitest version at execution time. Record the pick.
2. Add an axe scan to the E2E harness of task 19. The candidate is `@axe-core/webdriverio`. Confirm it at execution time.
3. Fail on serious and critical violations.
4. Scan the shell and the Settings screen now.
5. Add one line to the test-first plan of tasks 21, 23, 24, 25, and 32: each screen adds its own scan.
6. Add one keyboard-only E2E run that reaches each sidebar destination.

## Out of scope

- A manual screen-reader test
- The screens of tasks 21 to 25

## Test-first plan

1. A component test renders a button with no accessible name and expects the scan to report it.
2. The shell scan runs. Record its result before any fix.

Run the tests. They fail because no scan exists.

## Acceptance criteria

- [ ] A deliberately unlabeled button fails the component scan.
- [ ] The shell and Settings pass the scan in the built app.
- [ ] `pnpm verify` runs both scans.
- [ ] The new tests failed first for the expected reason.

## Verify

```powershell
pnpm test
pnpm test:e2e
pnpm verify
```
