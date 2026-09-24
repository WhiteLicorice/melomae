# 21 — Show the job queue with progress, ETA, and cancel

**Status:** TODO
**Phase:** 4 — Desktop UI
**Depends on:** 19
**SRS:** §5, §6
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
4. **Source interface (core requirement).** Show the source name and the stream codec and bitrate from `StreamInfo` on each job. Take the name from the source's `display_name()`. Do not hardcode a source. Flag the YouTube AAC fallback (task 13).
5. Show the clamp count (task 06) on the job.

## Out of scope

- The review queue (task 22)

## Test-first plan

1. Component tests driven by recorded event streams cover each stage, an error, an AAC fallback, and a job from a second fake source.
2. An E2E test cancels a running job and sees it leave the active list.

Run the tests. They fail because the screen does not exist.

## Acceptance criteria

- [ ] A cancel from the UI stops the job within one chunk.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
pnpm test:e2e
pnpm verify
```
