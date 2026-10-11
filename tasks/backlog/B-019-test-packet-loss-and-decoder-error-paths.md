# B-019 — Test packet loss and decoder error paths

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 03

## Impact

The task 03 MP4 fixtures place `moov` at the end, so a cut removes the metadata and the decode fails at the probe. No test cuts AAC or ALAC packet payload while the metadata stays intact. The decode loop in `decode/mod.rs` also skips a packet on a decoder `IoError` or `DecodeError`, and it ends decode on `ResetRequired`. No test covers those paths, so a damaged packet can drop audio with no error.

## Why this does not block MVP

The frame-count check rejects a decode that is shorter than the container declares. No review reproduced a silent wrong result through these paths.

## Expected behavior

Damaged or missing packets either return a typed error or are reported. A chained stream that needs a reset is decoded or rejected with a named error.

## Promotion condition

Promote this task when a user reports a damaged file that decoded with missing audio.

## Acceptance criteria

- [ ] A faststart MP4 (AAC and ALAC) with a cut payload returns a typed error.
- [ ] Tests exercise a decoder error packet and a `ResetRequired` stream, with a recorded red run.
