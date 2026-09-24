# 16 — Scan the library and deduplicate by MBID

**Status:** TODO
**Phase:** 3 — Library and pipeline
**Depends on:** 14, 15
**SRS:** §8
**Stack:** §F, §G
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Know which recordings the library already holds, so Melomae does not download them twice.

## Scope

1. Walk the library root. Read each audio file's MusicBrainz IDs with lofty.
2. Store the recording MBID, release MBID, path, and mtime in `library_items`.
3. Rescan incrementally. Read only files whose mtime changed. Remove rows for deleted files.
4. At queue time:
   - The same recording on the same release is "in library". Skip it.
   - The same recording on a different release is allowed. Flag it.

## Out of scope

- The "In library" badge UI (task 20)

## Test-first plan

1. Build a temporary library with tagged fixtures. A scan indexes every file.
2. An incremental rescan after one file changes reads only that file.
3. Both dedup rules hold.

Run the tests. They fail because the scanner does not exist.

## Acceptance criteria

- [ ] `Outcome` records the scan time for 1,000 generated files.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core library_scan
pnpm verify
```
