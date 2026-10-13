# B-039 — Compare the restore against the golden at the reference settings

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 07

## Impact

The CLI golden test compares an 18 s input at the adaptive chunk that the chooser picked on the gate machine. It does not cover the reference chunk settings, so it does not show that the output is the same across chunk sizes. Review 1 of task 07 found this gap.

## Why this does not block MVP

The measured output is well inside the SRS section 12.1 limits, and `Outcome` claims parity for that fixture and those settings only.

## Expected behavior

A test matrix compares the restore with the golden at the reference settings and at the adaptive settings, within the section 12.1 limits.

## Promotion condition

Promote this task with any change to the chunk chooser, the overlap, or the pad.

## Acceptance criteria

- [ ] The matrix passes at every listed setting.
