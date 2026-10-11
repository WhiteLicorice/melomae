# 34 — Make test temporary directories unique

**Status:** DONE
**Phase:** 1 — Restoration engine
**Depends on:** 03
**SRS:** §12
**Stack:** §K
**Assigned to:** Agent
**Started:** 2026-10-11 — Base commit `102f78c`, clean tree, `git lfs pull` in the worktree. Baseline gate at `102f78c`: board 62 records, tasks unittest 9 passed, Vitest 1 passed, cargo test 179 passed, pytest 19 passed.
**Outcome:** DONE 2026-10-11. `TempDir::new` (`crates/melomae-core/tests/setup/mod.rs:56`) now names its directory `<prefix>-<pid>-<stamp>-<sequence>`, where `sequence` is a process-wide `AtomicU64`, and creates it with `fs::create_dir`, so a repeated name errors instead of reusing a directory another test removes. Seven other clock-and-pid temporary helpers were fixed the same way: `TempDb::new` in `crates/melomae-core/tests/triage_scan.rs`, `crates/melomae-core/tests/triage_round1_fixes.rs`, `crates/melomae-core/tests/store_e2e.rs`, `crates/melomae-core/tests/store_regressions.rs`, `crates/melomae-core/tests/store_recovery.rs`, `crates/melomae-core/src/store/race_tests.rs`, and the `crates/melomae-core/src/store/mod.rs` tests module. Left unchanged: `crates/melomae-core/src/decode/mp4.rs:550` `write_temp` uses a fixed shared directory with a distinct file name per test, not a clock-or-pid name and no drop, so it is not the same pattern; `crates/melomae-core/src/encode/mod.rs:219` `temporary_path` is production code and out of scope, with an unproven collision risk for two concurrent writes to one target. Tests added in `crates/melomae-core/tests/temp_dir.rs`: `temp_dir_new_returns_distinct_paths_under_threads` (16 barrier-synchronized threads by 500 rounds, no panic and no repeated path; red 10 of 10 at `102f78c` with `6 of 8000 calls panicked`) and the e2e `e2e_parallel_fixture_builds_keep_separate_directories` (two concurrent `Fixtures::build()` keep separate directories). Gate at `0837b5a`: `pnpm verify` exit 0, board 62 records, tasks unittest 9 passed, Vitest 1 passed, cargo test 181 passed (baseline 179, +2 new tests), pytest 19 passed. Hosted CI on both operating systems is confirmed by the orchestrator after the branch push.

## Goal

Stop parallel tests from sharing, and deleting, one temporary fixture directory, so the gate passes on every run.

## Evidence

- Hosted CI run 38124727652 on `master` `3690b93` (2026-10-11): `windows-latest` failed three `round3_fixes` tests (`every_format_payload_cut_is_an_error`, `malformed_edit_list_does_not_fall_back`, `family_255_opus_names_itself_unsupported`). They panicked at `crates/melomae-core/tests/setup/mod.rs:253` (copy the awkward-named input: not found) and `:570` (ffmpeg: "Error opening input: No such file or directory" for `melomae-decode-7432-1791707400693618300\Florida!!!.opus`). `ubuntu-22.04` passed.
- The same two tests failed once in a local integration gate for task 04 on 2026-10-11, and passed when run alone.
- `TempDir::new` (`crates/melomae-core/tests/setup/mod.rs:51`) names the directory `<prefix>-<process id>-<nanoseconds since the epoch>`. On Windows the system clock advances in 100 ns steps (the failing path ends in `00`). Two tests in one test binary that call it in the same step get the same directory. The first test to finish removes it in `Drop` (`:73`) while the other still uses it. This cause is inferred from the code and the path. The reproduction test below confirms it.

## Scope

1. Make every `TempDir::new` call in a process return a new directory, for example with a process-wide atomic counter in the name, and create it so that an existing directory is an error, not a silent reuse.
2. Check the other temporary-directory helpers in `crates/melomae-core/tests/` and `crates/melomae-core/src` test modules for the same pattern, and fix each one.

## Out of scope

- Changes to the fixtures themselves.

## Test-first plan

1. A test that creates many `TempDir` values in a tight loop on several threads and asserts that every path is distinct. It fails on the current code (run it on Windows, where the clock step is 100 ns).

## Acceptance criteria

- [x] The new test failed first for the expected reason.
- [x] `cargo test --workspace` passes three times in a row on this machine with the default thread count.
- [ ] Hosted CI passes on both operating systems.
- [x] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core --test round3_fixes
pnpm verify
```
