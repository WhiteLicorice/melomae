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
6. The benchmark shows its progress and the elapsed time. When it fails, the flow shows a plain message that names the missing model or runtime, with Retry and Continue. A failed benchmark does not block the first run (SRS §3.1).
7. Do not add a Skip before a failure. SRS §3.3 says that the flow measures the speed. A Skip needs the owner decision in `tasks/ux/interface-proposal.md` section 11, item 8.
8. Each step has a heading. A step change moves the focus to the new heading and announces it once.
9. Use the output folder check from task 41 when that task is done.

`tasks/ux/interface-proposal.md` section 3.1 proposes the layout and the copy. It is a proposal until the owner accepts it.

## Out of scope

- The notices file itself (task 26)

## Test-first plan

1. Component tests cover each first-run step.
2. An E2E test with a fresh profile and no network completes the flow.
3. A component test forces a benchmark failure and completes the flow.
4. A copy test reads every string of the flow and the About screen. It fails on "lossless", "original quality", or "recovered" as a claim, and on the name of a download tool or a streaming service (scope items 3 and 4).
5. A component test completes the flow from the keyboard and checks the focus after each step change.

Run the tests. They fail because the flow does not exist.

## Acceptance criteria

- [ ] A fresh profile completes the flow in the built app with the network turned off.
- [ ] A forced benchmark failure still completes the first run.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
pnpm test:e2e
pnpm verify
```
