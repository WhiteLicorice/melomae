# 34 — Make test temporary directories unique

**Status:** TODO
**Phase:** 1 — Restoration engine
**Depends on:** 03
**SRS:** §12
**Stack:** §K
**Assigned to:** Agent
**Started:** —
**Outcome:** —

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

- [ ] The new test failed first for the expected reason.
- [ ] `cargo test --workspace` passes three times in a row on this machine with the default thread count.
- [ ] Hosted CI passes on both operating systems.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core --test round3_fixes
pnpm verify
```
