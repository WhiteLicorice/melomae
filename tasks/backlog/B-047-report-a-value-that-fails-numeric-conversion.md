# B-047 — Report a value that fails numeric conversion

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 15

## Impact

The support check accepts a mapped numeric field before the writer converts it, so a value that fails the conversion (for example a non-numeric track number into WAV) can drop. Review 1 of task 15 found this (finding 5). The write check now lists a dropped field, but the writer still drops it.

## Why this does not block MVP

The write check lists the field.

## Expected behavior

The writer keeps the value as text or lists it before the write, with a test.

## Promotion condition

Promote this task with the next change to numeric fields.

## Acceptance criteria

- [ ] A non-numeric track number is kept or listed in every format.
