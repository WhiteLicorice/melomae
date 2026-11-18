# 44 — Keep the triage scan running when a file or folder cannot be read

**Status:** DONE
**Phase:** 2 — Metadata, triage, and provenance
**Depends on:** 16
**SRS:** §4
**Stack:** §D
**Assigned to:** Agent
**Started:** `fd0606d`, clean worktree `C:\mt\w44` on branch `task/44`. The last recorded gate at `master` is task 35: board 66 records, tasks unittest 9, Vitest 1, cargo test 193 passed and 3 ignored, pytest 19. The triage suite baseline is in `C:\mt\reports\44-baseline-triage.txt`.
**Outcome:** DONE. `58cf171` keeps the scan running: a file whose read or `metadata` fails becomes an `unreadable` row with the operating system's text, an unopenable subfolder becomes a `ScanSkip` with the reason in `ScanReport::skipped_paths`, and the scan errors only when no root opens. Windows method: the tests deny a right with `icacls <path> /deny <account>:(RD)` for a folder and `(R)` for a file, on paths the test creates under `%TEMP%`; a `Drop` guard runs `icacls /remove:d` before the folder is deleted. Two of the four new scan tests were red first (`C:\mt\reports\44-red.txt`, 7 passed 2 failed): `a_folder_that_cannot_be_opened_is_a_recorded_skip_and_the_scan_continues` let `PermissionDenied (os 5)` escape `scan_roots`, and `a_scan_whose_only_root_is_missing_returns_an_error` returned `Ok` with all-zero counts. Both were renamed after that red run, to `triage_scan_records_an_unopenable_folder_and_continues` and `triage_scan_fails_when_no_root_opens`, so `44-red.txt` names the predecessors and its `ScanReport` debug has no `skipped_paths`. The corrupt-file and unopenable-file tests were green before the change, because a decode error or a denied read already became a row (scope 1 needed no code change there). No Windows red run covers the `metadata`-failure branch: a deny on `(RA)` does not stop `GetFileAttributes`, so the unit test `triage_skips_a_gone_file_not_stores_it` covers it. One later test (`records_a_missing_root_beside_a_readable_one`) was written with the fix, and `keeps_the_rows_of_an_unopenable_folder` is my reading of scope 2, not a line the task states. Gate `pnpm verify` exit 0 (`C:\mt\reports\44-verify.txt`, `RUST_TEST_THREADS=1`): board 119 records, tasks unittest 12, Vitest 1, cargo test 277 passed and 4 ignored (triage_scan 11), pytest 45. Two earlier gate runs were discarded: one failed because three `restore_signals` CLIs ran in parallel and ONNX gave `bad allocation`, and one passed against stale master artifacts in the withdrawn `C:\mt\shared`. The binary the final gate ran is `C:\mt\t44\debug\deps\triage_scan-599fa6bb03006640.exe`, which lists 11 tests.

**Outcome, round 2:** `50c5a7b` fixes the three findings of `C:\mt\reports\44-review.md`. B1: `walk` read each entry with `Path::is_dir`/`is_file`, which answer `false` to every `metadata` error, so a folder with mode `0400` listed its names and lost every file inside it — no row, no skip — and the removal pass deleted their stored rows. One `fs::metadata` match now recurses on a folder, collects an audio-named path that cannot be stat'ed so `visit` writes the `unreadable` row, ignores `NotFound`, and records a `ScanSkip` for any other non-audio failure. The new test `triage_scan_reports_the_files_of_a_folder_it_cannot_traverse` is `#[cfg(unix)]` and chmods a folder `0400`; this machine cannot run it, because WSL has no `cargo` and rustup holds only the Windows target, and the reviewer's `C:\mt\probe44.sh` is not on disk. `C:\mt\probe44-round2.sh` therefore rebuilds that measurement as uid 1000 and prints the old walk removing the stored row with no row written where the new walk writes `unreadable (Permission denied)` and keeps the row (`C:\mt\reports\44-red2.txt`). N1: the removal pass held a row back for any skip path, so a deleted file root kept its verdict forever; it now holds rows back only while the skip path is still a folder. N2: `Visit::Gone` left its path in `present_set`, so the row survived one extra scan; the set now holds only the paths the scan read or found unchanged. N1 and N2 were red first in `44-red2.txt` (`removed 0`, expected 1), the N2 test through the new private `scan_roots_with` seam. Residual: a folder whose own `metadata` fails is no longer protected by the N1 rule, because the rule asks the same `metadata`; no test reaches that shape. Gate `pnpm verify` for round 2: `C:\mt\reports\44-verify2.txt`, see the report.

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

- [x] One unreadable file gives one `unreadable` row and does not stop the scan.
- [x] One unreadable subfolder gives one recorded skip and does not stop the scan.
- [x] The new tests failed first for the expected reason, or `Outcome` says why not.
- [x] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core triage
pnpm verify
```
