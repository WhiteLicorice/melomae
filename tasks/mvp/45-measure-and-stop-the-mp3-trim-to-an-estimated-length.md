# 45 — Measure and stop the MP3 trim to an estimated length

**Status:** TODO
**Phase:** 1 — Restoration engine
**Depends on:** 03
**SRS:** §6
**Stack:** §D
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Find out whether an MP3 with no frame-count header loses audio at its end, and stop the loss if it does.

## Evidence

The ponytail audit of 2026-11-11 read the code at `master` `23f0bcf`. It built and ran nothing. The orchestrator read each cited line again on the same day.

- `crates/melomae-core/src/decode/mod.rs:320-324`: `valid_frames` comes from the track duration when the time base is one tick for each sample.
- `crates/melomae-core/src/decode/mod.rs:587-591`: with no MP4 edit list, `end` is `valid_frames` capped at the decoded count. The decoder cuts the audio there.
- `crates/melomae-core/src/decode/mod.rs:325-339`: the comment says that Symphonia estimates the frame count of an MP3 with no Xing, Info, or VBRI header. `declared_frames` ignores the estimate, and `valid_frames` does not.
- The audit cites `symphonia-bundle-mp3-0.6.1/src/demuxer.rs:459-477` and `:683-733` for the estimate. The orchestrator did not open that file.
- Inferred, not measured: the estimate is lower than the real count for some files, so the decoder drops the end of the audio.
- B-016 covers a different defect: the detection of a truncated MP3 with no header.

## Scope

1. Measure first. Decode a headerless CBR MP3 and a headerless VBR MP3, and compare each frame count with the ffmpeg oracle of task 03. Do the same with a terminal APEv2 tag.
2. If no file loses audio, record the numbers in `Outcome` and close the task with no code change.
3. If a file loses audio, use the track duration as a trim bound only when `mp3_declares_frame_count` is true. A headerless MP3 keeps its decoded frames.
4. An MP3 with a header still loses its encoder padding.

## Out of scope

- Truncation detection for a headerless MP3 (backlog B-016)

## Test-first plan

1. A headerless VBR MP3 decodes to the frame count of the ffmpeg oracle, within one MP3 frame.
2. The same file with a terminal APEv2 tag decodes to the same count.
3. An MP3 with a Xing header keeps its current decoded count.

Run the tests. Record whether tests 1 and 2 fail. A pass means that the inferred loss does not exist.

## Acceptance criteria

- [ ] `Outcome` holds the measured frame counts for each fixture, before any change.
- [ ] No headerless fixture loses more than one MP3 frame against the oracle.
- [ ] Every task 03 fixture keeps its decoded count.
- [ ] The new tests failed first for the expected reason, or `Outcome` says why not.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core decode
pnpm verify
```
