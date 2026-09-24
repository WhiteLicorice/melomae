# 25 — Add the first-run setup and the attribution screen

**Status:** TODO
**Phase:** 4 — Desktop UI
**Depends on:** 08, 11, 19
**SRS:** §1, §3, §11
**Stack:** §A, §E
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Take a new user from install to a working app, and credit Apollo as its license requires.

## Scope

1. The first-run flow, in this order:
   1. Show the Apollo caveat (SRS §1.3).
   2. Show the user-responsibility notice for YouTube's Terms of Service and copyright (SRS §11.5).
   3. Ask for the library folder.
   4. Download the tools (task 11) with visible progress. On failure, explain which features still work.
   5. Run a 2 s restore benchmark. Show the expected minutes for a 4-minute track on CPU, and on GPU when one exists.
2. An About screen:
   - the Apollo credit per CC BY-SA 4.0 §3(a): the creators, the license name and link, the source link, and the modifications
   - the no-endorsement statement (SRS §11.4)
   - the Melomae license (GPL-3.0-or-later)
   - a link to the third-party notices (task 26)
3. The copy never claims "lossless", "original quality", or "recovered".

## Out of scope

- The notices file itself (task 26)

## Test-first plan

1. Component tests cover each first-run step and the tool-failure branch.
2. An E2E test with a fresh profile completes the flow.

Run the tests. They fail because the flow does not exist.

## Acceptance criteria

- [ ] A fresh profile completes the flow in the built app.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
pnpm test:e2e
pnpm verify
```
