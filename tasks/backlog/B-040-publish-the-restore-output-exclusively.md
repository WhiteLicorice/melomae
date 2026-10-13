# B-040 — Publish the restore output exclusively

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 07

## Impact

The CLI preflight checks that the output does not exist before decode, and the encoder renames its temporary file at the end. Another writer can create the output between the two steps, and the rename then replaces that file. Review 1 of task 07 named this concern. It is inferred, not reproduced.

## Why this does not block MVP

A competing writer on the same output path is not in the MVP threat model.

## Expected behavior

The final publication fails when the target exists at rename time, and a test with a concurrent writer proves it.

## Promotion condition

Promote this task when the app restores into folders that other programs write at the same time.

## Acceptance criteria

- [ ] The concurrent-writer test passes, and the existing file stays unchanged.
