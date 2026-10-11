# B-017 — Support Opus mapping family 255 and unassigned channels

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 03

## Impact

Task 03 decodes Opus mapping families 0 and 1. A family-255 file returns `UnsupportedFormat` with a reason that names the family. A legal header whose stream counts exceed the channel count (an unassigned decoded channel, RFC 7845 §5.1.1) also returns `UnsupportedFormat`. Symphonia's Ogg identification parser rejects family 255 before the decoder runs, and the opusic-c multistream binding requires the stream count plus the coupled count to be no greater than the channel count.

## Why this does not block MVP

Music files use families 0 and 1. The decoder names the unsupported mapping instead of failing silently.

## Expected behavior

Family 255 up to 255 channels and unassigned decoded channels decode and match the libopus oracle, in Ogg and WebM.

## Promotion condition

Promote this task when a user file with family 255 or unassigned channels appears.

## Acceptance criteria

- [ ] A family-255 Ogg and WebM file decodes through an Ogg identification adapter, with no patch to Symphonia.
- [ ] A header with an unassigned decoded channel decodes.
- [ ] Tests compare each channel with the libopus oracle.
