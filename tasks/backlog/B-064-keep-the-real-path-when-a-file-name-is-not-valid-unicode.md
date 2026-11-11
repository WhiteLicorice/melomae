# B-064 — Keep the real path when a file name is not valid Unicode

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 16

## Impact

Two files whose names differ only in invalid bytes get the same stored path. The second row replaces the first one. A rescan cannot match either file, and the removal step can delete the row of a file that still exists.

## Evidence

The ponytail audit of 2026-11-11 read the code at `master` `23f0bcf`. It built and ran nothing. The orchestrator read each cited line again on the same day.

- `crates/melomae-core/src/triage/mod.rs:112`: the stored path is `path.to_string_lossy()`.
- `crates/melomae-core/src/triage/mod.rs:181-183`: the rescan key is the same lossy string.
- Nobody reproduced the collision. It needs two such names in one folder.

## Why this does not block MVP

Such names are rare. Windows paths from a normal library are valid Unicode. Linux and mounted drives allow the bad names.

## Expected behavior

A file whose name is not valid Unicode gets an `unreadable` row with that reason, and the scan never uses a lossy string as a key. A later change can store the path as bytes.

## Promotion condition

Promote this task before the Linux release, or when a user reports a missing row.

## Acceptance criteria

- [ ] On Linux, two files whose names differ only in invalid bytes get two rows.
- [ ] A rescan does not delete either row.
- [ ] A test fails before the change and passes after it.
