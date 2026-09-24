# 12 — Match MusicBrainz tracks to YouTube Music candidates

**Status:** TODO
**Phase:** 2 — Metadata and acquisition
**Depends on:** 02, 09, 11, 30
**SRS:** §5
**Stack:** §E, §F
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

For each MusicBrainz track, find the YouTube Music candidate that is the same recording, and say how sure the match is.

## Evidence

Task 02 saved scrubbed search results for 50 recordings as fixtures. Read its evidence file for the available fields.

## Scope

**Source interface (core requirement).** This task has two parts. The search belongs to the YouTube Music source. The matcher is source-neutral. It scores `Candidate` values from any source and never calls yt-dlp. The task 30 boundary test enforces this.

1. Implement `YouTubeMusicSource::search` in `source/youtube/`. It searches YouTube Music through yt-dlp, the way task 02 proved, and maps each result to a `Candidate`. A song result from a "Topic" channel maps to `official_audio = true`.
2. Score each candidate in the source-neutral matcher:
   - Duration gate: |Δ| ≤ max(3 s, 2%) of the MusicBrainz duration. A candidate outside the gate cannot be `matched`.
   - Normalized title and artist similarity. Remove case, punctuation, "feat." credits, and bracketed noise before the comparison.
   - A penalty for live, remix, edit, radio edit, instrumental, karaoke, cover, and sped-up. The penalty does not apply when the MusicBrainz title or disambiguation contains the same word.
   - A preference for candidates with `official_audio = true`.
3. Classify the track as `matched`, `uncertain`, or `none`. Return the ranked candidates for the review queue.

## Out of scope

- The review UI (task 22)
- MusicBrainz streaming links (backlog B-007)

## Test-first plan

1. Write table tests over the task 02 fixtures.
2. Run the same matcher tests on `FakeSource` candidates (task 30). This proves the matcher does not depend on YouTube.

Run the tests. They fail because the matcher does not exist.

## Acceptance criteria

- [ ] On the task 02 sample, auto-accepted matches are correct at 98% precision or better.
- [ ] At least 70% of the sample tracks auto-accept.
- [ ] These targets are inferred. If the fixtures show they are wrong, recalibrate them and record why.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core matcher
pnpm verify
```
