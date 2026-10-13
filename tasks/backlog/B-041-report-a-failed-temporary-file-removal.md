# B-041 — Report a failed temporary file removal

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 06

## Impact

`crates/melomae-core/src/encode/mod.rs:207`, `:211`, `:217`, and `:222` remove the temporary file with `let _ = fs::remove_file(..)`. When the delete fails, for example because another program holds the file, the error is lost and the temporary file stays with no message. Review 2 of task 07 named this flaw. It was not injected.

## Why this does not block MVP

On a normal cancel or error the delete succeeds, and the reviewers saw no temporary file left.

## Expected behavior

A failed delete is reported to the caller, or logged with the path, and a test that holds the file open proves it.

## Promotion condition

Promote this task with B-022, or when a user reports a leftover temporary file.

## Acceptance criteria

- [ ] A test with a held temporary file sees the report, and the normal paths stay unchanged.
