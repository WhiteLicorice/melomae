# B-026 — Handle directory cycles in the triage scan

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 16

## Impact

The folder walk in `crates/melomae-core/src/triage/mod.rs` (near `fs::read_dir`, line 422) follows directories without tracking the directories it already visited. A symbolic link or junction that points to a parent folder can repeat the walk. The first task 16 review found this (recommendation 5). No cycle was reproduced.

## Why this does not block MVP

Users usually drop music folders without link cycles.

## Expected behavior

The scan records each visited directory's identity and skips a directory it already walked. The link policy is documented.

## Promotion condition

Promote this task with the triage screen (task 24), or when a user reports a stuck scan.

## Acceptance criteria

- [ ] A test with a link cycle finishes and lists each file once, and fails or hangs before the change.
