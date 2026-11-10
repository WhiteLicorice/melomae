# 21 — Show the job queue with progress, ETA, and cancel

**Status:** TODO
**Phase:** 4 — Desktop UI
**Depends on:** 19
**SRS:** §4, §6, §9
**Stack:** §A
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Show every job's stage and progress, and let the user control it.

## Evidence

On CPU, a 4-minute song restores in about 12.5 min on the dev machine (Stack §K). The ETA must be honest about this.

## Scope

1. Show each job's stage and progress. A restore shows chunk n of m.
2. Compute the ETA from the RTF measured during the current job.
3. Provide cancel, retry, open file location, and error details.
4. Show the input codec, bitrate, and triage verdict on each job.
5. Show the clamp count (task 06) on the job.
6. Show an empty state with a link to the Restore screen.
7. Show a connecting state before the first event. When events stop for a job that still runs, say so and offer a retry. Record the chosen timeout in `Outcome`.
8. Give the progress bar `role="progressbar"` with its value. A polite live region announces a stage change once. It does not announce each chunk.
9. Virtualize the list with TanStack Virtual (Stack §A). Apply progress events at most once per animation frame. Show the active, waiting, done, and failed counts outside the list.
10. Show the user message of a failed job first (task 18). Put the technical detail behind a disclosure.
11. A cancel moves the focus to the next job, or to the empty state.

`tasks/ux/interface-proposal.md` section 3.3 proposes the row layout and the copy. It is a proposal until the owner accepts it.

## Out of scope

- The triage screen (task 24)

## Test-first plan

1. Component tests driven by recorded event streams cover each stage, an error, and a job with restore off.
2. An E2E test cancels a running job and sees it leave the active list.
3. Component tests cover the empty queue, the connecting state, and a recorded stream that stops.
4. A component test with a recorded 500-job stream renders only the visible rows.
5. A component test finds the progress bar by its role and reads its value.

Run the tests. They fail because the screen does not exist.

## Acceptance criteria

- [ ] A cancel from the UI stops the job within one chunk.
- [ ] No raw Rust error text shows on a failed job.
- [ ] `Outcome` records a profile of the 500-job stream.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
pnpm test:e2e
pnpm verify
```
