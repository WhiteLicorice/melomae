# 49 — Make the Xing MP3 test detect the padding trim

**Status:** TODO
**Phase:** 1 — Restoration engine
**Depends on:** 45
**SRS:** §6
**Stack:** §C
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Make the Xing-header MP3 control test fail when the decoder stops trimming the encoder delay and padding.

## Evidence

- The task 45 review (2026-11-18) found that the control test (`crates/melomae-core/tests/decode_formats.rs:383-396`) asserts only `decoded.frames() == reference_frames`.
- With the whole gapless trim disabled (`let gapless = false;` in place of `let gapless = count_declared;`), the decoded count stays 264600, because the container's declared duration supplies the same bound. So the test passes without the trim it names.
- The review measured the trim on that fixture: the decoded output matches the ffmpeg oracle sample for sample (maximum absolute difference 4e-6), and the same comparison shifted by the 576-sample delay gives 1.187.

## Scope

1. Compare the decoded samples with the ffmpeg reference in that test, not only the frame count. A shift by the encoder delay must fail it.

## Out of scope

- Any change to the decoder.

## Test-first plan

1. With the trim disabled in a scratch copy, the strengthened test fails.
2. With the trim in place, it passes.

## Acceptance criteria

- [ ] The Xing control test compares samples against the ffmpeg reference.
- [ ] It fails with the gapless trim disabled.
- [ ] `pnpm verify` passes.
