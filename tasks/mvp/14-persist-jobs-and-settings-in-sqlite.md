# 14 — Persist jobs and settings in SQLite

**Status:** DONE
**Phase:** 3 — Jobs and pipeline
**Depends on:** 00
**SRS:** §4, §6, §8, §9
**Stack:** §G
**Assigned to:** Agent
**Started:** 2026-10-10 — Base commit fd62a4e, clean tree. Baseline gate: board 45 records, tasks unittest 9 passed, Vitest 1 passed, cargo test 1 passed, pytest 8 passed in about 7 minutes.
**Outcome:** DONE 2026-10-10. Fix round 1 fixed the four review findings. Fix round 2 made the two concurrency regressions deterministic, with no production change. `rusqlite` 0.40.2 with the bundled SQLite. Default features are off, so the wasm and vfs crates are not pulled in. The store is `crates/melomae-core/src/store/mod.rs`, with unit tests beside the code, the concurrency regressions in `src/store/race_tests.rs`, and integration tests in `tests/store_e2e.rs`, `tests/store_regressions.rs`, and `tests/store_recovery.rs`. Tables: `settings`, `jobs`, `triage_items`, and `history`. Migration 1 sets `PRAGMA user_version` to 1 inside one transaction, and each migration is guarded by its own version. `open` opens and migrates only. It rejects a `user_version` above the supported version with `StoreError::UnsupportedSchema { found, supported }` before any write. `recover_interrupted_jobs` moves jobs in `restoring`, `encoding`, or `tagging` back to `queued` and returns the count. The app calls it once at startup, and `open` never touches the jobs. `transition_job` validates the state and the `restore_enabled` flag, and writes with a conditional `UPDATE ... WHERE id = ?1 AND state = ?2`, so a concurrent change is never overwritten. A queued job with restore on moves only to `restoring`, with restore off only to `encoding`, and `queued -> failed` and `queued -> cancelled` stay legal for both. Every other move returns `StoreError::IllegalTransition`. `triage_items` holds the task 16 fields. `history` holds the seven SRS §8.8 fields. Gate: `pnpm verify` exit 0. Board 45 records, tasks unittest 9 passed, Vitest 1 passed, cargo test 16 passed (melomae-core 7 unit, 1 no-tauri, 1 e2e, 2 recovery, 5 regression), pytest 8 passed in 359 s. Red-first, round 1: the new tests failed against `9c7d1e9` for each finding, and `tests/store_recovery.rs` did not compile there. Red-first, round 2: the two `src/store/race_tests.rs` tests drive the SQLite busy callback so the store's write cannot be read before its own read; both failed on `9c7d1e9` and pass at this head. Red-first, round 0: `cargo test -p melomae-core store` failed with E0432 unresolved imports of `Store`, `JobState`, `HistoryRow`, and `HistoryResult`. The round-0 gate first failed at Vitest with `React.act is not a function`, because the shell set NODE_ENV=production. Removing NODE_ENV made it pass. Note: no API writes `jobs.output_path` or `jobs.error` yet. Task 17 sets them.

## Goal

Keep settings, jobs, triage results, and history in one SQLite database so work survives an app restart.

## Scope

1. Use `rusqlite` with the bundled SQLite. Put the database in the app data directory. The caller passes the path.
2. Number migrations with `PRAGMA user_version`.
3. Tables: `settings`, `jobs`, `triage_items`, and `history`.
4. The job state machine:
   - queued → restoring → encoding → tagging → done
   - `failed` and `cancelled` are exits from any active state
   - a job with restore off goes from `queued` to `encoding`
5. Reject an illegal transition with a typed error.
6. On start, move each job that was in flight back to `queued`.
7. `triage_items` holds the input path, size, modification time, and the task 16 measurements and verdict.
8. `history` holds the fields in SRS §8.8.

## Out of scope

- The pipeline itself (task 17)
- Triage measurements (task 16)

## Test-first plan

1. A transition-table test accepts every legal transition and rejects every illegal one.
2. A migration test builds the schema from an empty file. It checks the `triage_items` and `history` columns.
3. A restart test puts a job in `restoring`, reopens the store, and finds the job in `queued`.

Run the tests. They fail because the store does not exist.

## Acceptance criteria

- [x] The three tests pass.
- [x] The new tests failed first for the expected reason.
- [x] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core store
pnpm verify
```
