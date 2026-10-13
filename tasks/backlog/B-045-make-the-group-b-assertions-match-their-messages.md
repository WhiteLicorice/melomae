# B-045 — Make the Group B assertions match their messages

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 15

## Impact

The Group B no-silent-loss test asserts less than its messages say. Review 4 of task 15 found this (finding 4). Review 6 confirmed that the exact native frames survive for the eight current cases.

## Why this does not block MVP

The exact-frame comparison passes today.

## Expected behavior

Each assertion checks what its message claims.

## Promotion condition

Promote this task with B-046.

## Acceptance criteria

- [ ] The assertions fail when a Group B frame changes.
