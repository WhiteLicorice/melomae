# B-007 — Use MusicBrainz streaming links as the first match source

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 12, 30

## Impact

Some MusicBrainz recordings and releases carry URL relationships to YouTube. An editor already confirmed each link, so a link can be more accurate than a search.

## Why this does not block MVP

Task 12's search matcher covers every track. Link coverage in MusicBrainz is partial. This is inferred and not yet measured.

## Expected behavior

Before it searches, the matcher checks the URL relationships on the recording and the release. MusicBrainz links point to YouTube, and also to Bandcamp and SoundCloud. `SourceRegistry.claims_url` (task 30) routes each link to the source that supports it. A link with no registered source is ignored. A linked candidate that passes the duration gate becomes the `matched` candidate.

## Promotion condition

Promote this task when task 12's measured precision falls below its target, or when a sample shows useful link coverage.

## Acceptance criteria

- [ ] `Outcome` records the link coverage on the task 02 sample.
- [ ] Linked candidates still pass the duration gate.
