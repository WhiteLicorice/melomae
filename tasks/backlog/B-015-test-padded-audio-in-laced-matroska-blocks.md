# B-015 — Test padded audio in laced Matroska blocks

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 03

## Impact

The WebM trim applies each block's `DiscardPadding` to the frames that the Matroska scanner counts for that block. The tests check the lace count of an EBML-laced block and the trim of single-packet blocks. No test decodes real audio from a padded laced block. If the scanner's packet count and Symphonia's demuxed packet count differ for a laced block, the trim removes the wrong samples. The task 03 reviews named this as the most likely untested trim defect.

## Why this does not block MVP

The common encoders (ffmpeg, libopus tools) write one Opus packet per WebM block. No review reproduced a wrong result.

## Expected behavior

A padded laced block trims exactly its own padding, for Xiph, fixed-size, and EBML lacing.

## Promotion condition

Promote this task when a user file with laced Opus blocks appears, or with the next change to `decode/matroska.rs`.

## Acceptance criteria

- [ ] One public test per lace mode decodes a padded laced block and matches the libopus oracle within the settled tolerance.
- [ ] A recorded red run shows each test can fail.
