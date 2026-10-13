# B-046 — Map the Group B tag frames

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 15

## Impact

Under the round-3 ruling, TIPL roles, TMCL, POPM, and the AcoustID fingerprint are only protected against silent loss. They are not mapped between ID3 and Vorbis the way Picard maps them.

## Why this does not block MVP

Each frame is either carried or listed, so nothing is lost silently.

## Expected behavior

Each Group B frame maps to its Picard field in both directions, with tests.

## Promotion condition

Promote this task when a user needs these fields in another format.

## Acceptance criteria

- [ ] A round trip through FLAC, Opus, and WAV keeps each Group B field.
