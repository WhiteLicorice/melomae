# B-048 — Carry the payloads of mapped frames

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 15

## Impact

A mapped frame ID does not guarantee that its payload is extracted, for example UFID and other binary frames. Review 1 of task 15 found this (finding 6).

## Why this does not block MVP

The fixture frames survive, and the carried frames pass the exact-frame test.

## Expected behavior

Each mapped frame's payload is extracted and written, or the frame is listed.

## Promotion condition

Promote this task when a user file shows a lost payload.

## Acceptance criteria

- [ ] A test per mapped binary frame keeps its payload.
