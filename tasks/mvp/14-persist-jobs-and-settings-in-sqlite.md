# 14 — Persist jobs and settings in SQLite

**Status:** TODO
**Phase:** 3 — Jobs and pipeline
**Depends on:** 00
**SRS:** §4, §6, §8, §9
**Stack:** §G
**Assigned to:** Agent
**Started:** —
**Outcome:** —

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

- [ ] The three tests pass.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core store
pnpm verify
```
