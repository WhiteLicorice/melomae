# 14 — Persist jobs and settings in SQLite

**Status:** TODO
**Phase:** 3 — Library and pipeline
**Depends on:** 00, 30
**SRS:** §5, §6
**Stack:** §G
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Keep settings, jobs, and the library index in one SQLite database so work survives an app restart.

## Scope

1. Use `rusqlite` with the bundled SQLite. Put the database in the app data directory. The caller passes the path.
2. Number migrations with `PRAGMA user_version`.
3. Tables: `settings`, `jobs`, and `library_items`.
4. The job state machine:
   - queued → matching → review → downloading → restoring → encoding → tagging → done
   - `failed` and `cancelled` are exits from any active state
   - `review` returns to `downloading` after the user decides
5. Reject an illegal transition with a typed error.
6. On start, move each job that was in flight to its last safe state. A half-downloaded job returns to `queued`.
7. **Source interface (core requirement).** Each job stores the source ID, the source URL, and the fetched `StreamInfo` (task 30). Nothing in the schema names a specific source.

## Out of scope

- The pipeline itself (task 17)

## Test-first plan

1. A transition-table test accepts every legal transition and rejects every illegal one.
2. A migration test builds the schema from an empty file. It checks the source ID, source URL, and `StreamInfo` columns.
3. A restart test puts a job in `restoring`, reopens the store, and finds the job back in a safe state.

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
