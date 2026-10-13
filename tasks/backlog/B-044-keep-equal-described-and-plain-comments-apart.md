# B-044 — Keep equal described and plain comments apart

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 15

## Impact

When a described comment and the plain comment have equal bodies, the WAV writer erases the plain comment. Review 4 of task 15 found this (finding 2).

## Why this does not block MVP

It needs two comments with the same text, which is rare.

## Expected behavior

Both comments survive with their descriptions.

## Promotion condition

Promote this task with the next change to comment mapping.

## Acceptance criteria

- [ ] A test with equal bodies keeps both comments.
