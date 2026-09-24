# 17 — Run the job pipeline end to end

**Status:** TODO
**Phase:** 3 — Library and pipeline
**Depends on:** 07, 13, 14, 15, 16, 30
**SRS:** §5, §6, §7, §8
**Stack:** §B, §G
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Run each queued track through every stage, from match to library file, with a durable state and a typed event stream.

## Scope

1. Run the stages in this order for each job:
   1. match (task 12)
   2. review, only when the match is `uncertain` or `none`
   3. download (task 13)
   4. decode (task 03)
   5. restore (tasks 04 and 05), unless the job has restore turned off
   6. encode (task 06)
   7. tag and file (task 15)
   8. move into the library atomically
2. Run one restore worker and at most 2 downloads at a time.
3. Keep the downloaded source in the cache directory so task 23 can compare it. Respect the "keep sources" setting.
4. Emit typed events: job ID, stage, progress, ETA, and message.
5. Isolate failures. One failed job does not stop the queue. A failed job can be retried.
6. Remove each job's temporary files when it ends.
7. **Source interface (core requirement).** The pipeline gets audio only through `SourceRegistry` (task 30). It never names a specific source.
8. The restore stage applies SRS §6.10. A lossless `StreamInfo` skips restore by default. A forced restore overrides the skip.

## Out of scope

- UI (Phase 4)

## Test-first plan

1. Fake-stage orchestration tests: stage order, a cancel during a stage, failure isolation, and resume after restart.
2. A real run through `SourceRegistry` with `LocalFileSource` (task 30) produces a tagged FLAC in a temporary library.
3. A `FakeSource` lossless stream skips restore. The same stream with a forced restore runs restore.

Run the tests. They fail because the orchestrator does not exist.

## Acceptance criteria

- [ ] The real run produces a tagged, restored FLAC at the correct library path.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core pipeline
pnpm verify
```
