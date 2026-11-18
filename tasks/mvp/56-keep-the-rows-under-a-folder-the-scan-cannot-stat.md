# 56 — Keep the rows under a folder the scan cannot stat

**Status:** TODO
**Phase:** 2 — Metadata, triage, and provenance
**Depends on:** 44
**SRS:** §4
**Stack:** §D
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Keep the stored rows under a folder whose `metadata` call fails, because the folder still exists.

## Evidence

- The task 44 round 2 review (finding 2): the removal pass protects a skipped path only while `path.is_dir()` succeeds (`crates/melomae-core/src/triage/mod.rs:232`). A folder inside a listable but untraversable folder fails `metadata`, becomes a `ScanSkip` (`:644-649`), fails `is_dir()`, and loses every stored row under it.
- A WSL probe as uid 1000 with a real `0400` folder removed `locked/sub/deep.wav` while the file still existed (`C:\mt\probe44-r2-review.sh`).

## Scope

1. Record on `ScanSkip` whether the path was `NotFound`, from the error kind in `collect_files`.
2. Protect the rows under every skip that is not `NotFound`, in place of the `is_dir()` test.

## Test-first plan

1. A `#[cfg(unix)]` test builds the nested `0400` shape and expects the deep row to stay. It fails before the fix.
2. `triage_scan_removes_the_row_of_a_deleted_file_root` still passes.

## Acceptance criteria

- [ ] The rows under an unstattable folder stay, and the rows of a deleted root go.
- [ ] The unix test ran and failed first on a Linux machine, with the log in the task record.
- [ ] `pnpm verify` passes.
