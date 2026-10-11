# B-028 — Make the encode temporary file name unique

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 03

## Impact

`temporary_path` (`crates/melomae-core/src/encode/mod.rs:219`) names the hidden sibling file `.<target>.melomae-<pid>-<nanoseconds>.tmp`, and `write_stream` (`:207`) opens it with `File::create`, which truncates an existing file. Two writes to the same target in one process within one clock step (100 ns on Windows) get the same temporary file. The task 34 executor and reviewer found this. The risk is inferred from the code. No collision was reproduced.

## Why this does not block MVP

The target file name is in the temporary name, so a collision needs two concurrent writes to one target file. The job queue (task 17) gives each output its own target.

## Expected behavior

The temporary name carries a process-wide counter, and the file is opened with exclusive creation (`create_new`), so a collision is an error.

## Promotion condition

Promote this task with the job queue (task 17), or when two writes to one target can run at once.

## Acceptance criteria

- [ ] A test that asks for two temporary paths for one target in a row gets two different paths, and fails before the change.
