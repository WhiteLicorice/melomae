# 25 — Add the first-run setup and the attribution screen

**Status:** TODO
**Phase:** 4 — Desktop UI
**Depends on:** 08, 19
**SRS:** §1, §3, §11
**Stack:** §A
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Take a new user from install to a working app, and credit Apollo as its license requires.

## Scope

1. The first-run flow, in this order:
   1. Show the Apollo caveat (SRS §1.3).
   2. Ask for the default output folder.
   3. Run a 2 s restore benchmark. Show the expected minutes for a 4-minute track on CPU, and on GPU when one exists.
2. An About screen:
   - the Apollo credit per CC BY-SA 4.0 §3(a): the creators, the license name and link, the source link, and the modifications
   - the no-endorsement statement (SRS §11.4)
   - the Melomae license (GPL-3.0-only)
   - a link to the third-party notices (task 26)
3. The copy never claims "lossless", "original quality", or "recovered".
4. The copy never names or suggests a download tool or streaming service (SRS §1.5).
5. The flow makes no network request (SRS §3.4).

## Out of scope

- The notices file itself (task 26)

## Test-first plan

1. Component tests cover each first-run step.
2. An E2E test with a fresh profile and no network completes the flow.

Run the tests. They fail because the flow does not exist.

## Acceptance criteria

- [ ] A fresh profile completes the flow in the built app with the network turned off.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
pnpm test:e2e
pnpm verify
```
