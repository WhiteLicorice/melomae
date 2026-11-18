# 55 — Retry an unreadable file on the next scan

**Status:** TODO
**Phase:** 2 — Metadata, triage, and provenance
**Depends on:** 44
**SRS:** §4
**Stack:** §D
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Give a verdict to a file that was unreadable and is readable again, on the next scan.

## Evidence

- The task 44 round 2 review (2026-11-18, `.claygo/mt-evidence/reports/44-review.md` finding 1): a read failure stores an unreadable row with the file's real size and time (`crates/melomae-core/src/triage/mod.rs:295-300`). The next scan sees the same size and time and returns `Unchanged` (`:284-288`) without a read.
- A scratch test denied `(R)`, scanned, lifted the deny, and scanned again. The verdict stayed `Unreadable`, with `scanned=0 skipped=1`.

## Scope

1. Skip the `Unchanged` shortcut when the stored verdict is `Unreadable`, so each scan reads the file again.

## Test-first plan

1. A Windows test denies `(R)`, scans, lifts the deny, and scans. It expects a real verdict. It fails before the fix.

## Acceptance criteria

- [ ] A file that is readable again gets a real verdict on the next scan.
- [ ] The new test failed first.
- [ ] `pnpm verify` passes.
