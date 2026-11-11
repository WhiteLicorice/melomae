# 44 — Keep the triage scan running when a file or folder cannot be read

**Status:** TODO
**Phase:** 2 — Metadata, triage, and provenance
**Depends on:** 16
**SRS:** §4
**Stack:** §D
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Give the user a verdict for every readable file when one file or one folder in the library cannot be read.

## Evidence

The ponytail audit of 2026-11-11 read the code at `master` `23f0bcf`. It built and ran nothing. The orchestrator read each cited line again on the same day.

- `crates/melomae-core/src/triage/mod.rs:175-178`: a failed `fs::metadata` returns an error from the scan loop.
- `crates/melomae-core/src/triage/mod.rs:190`: `triage_file(path)?` returns an error from the scan loop.
- `crates/melomae-core/src/triage/mod.rs:105-108`: `triage_file` returns an error, not a verdict, when `metadata` fails.
- `crates/melomae-core/src/triage/mod.rs:422-431`: a failed `read_dir`, or one failed entry, stops the folder walk.
- SRS §4.3 defines an `unreadable` verdict that shows the reason (`docs/melomae-srs.md:43`). Task 24 shows that reason on the row.
- Nobody reproduced the failure. The claim comes from the error propagation in the code.

## Scope

1. A file whose `metadata` or triage fails gets an `unreadable` row with the error text. The scan continues.
2. A subfolder that cannot be opened becomes a recorded skip in the scan report. The walk continues with the other folders.
3. A file that disappears between the walk and the read is skipped.
4. The scan returns an error only when no root can be opened.

## Out of scope

- A directory cycle in the walk (backlog B-026)

## Test-first plan

1. A scan of a folder with one corrupt file and one good file returns two rows. Confirm first that this test is red: if the decode error already becomes a row, use a file that the process cannot open.
2. A scan of a tree with one subfolder that cannot be opened returns the rows of the other folders and one recorded skip. On Windows, build the case with an ACL or a locked handle. State the method in `Outcome`.
3. A scan whose only root does not exist returns an error.

Run the tests. They fail because the first error stops the scan.

## Acceptance criteria

- [ ] One unreadable file gives one `unreadable` row and does not stop the scan.
- [ ] One unreadable subfolder gives one recorded skip and does not stop the scan.
- [ ] The new tests failed first for the expected reason, or `Outcome` says why not.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core triage
pnpm verify
```
