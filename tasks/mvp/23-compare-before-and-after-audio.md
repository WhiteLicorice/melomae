# 23 — Compare before and after audio

**Status:** TODO
**Phase:** 4 — Desktop UI
**Depends on:** 21
**SRS:** §10
**Stack:** §H
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Let the user hear the source and the restored file, and switch between them at the same position.

## Evidence

Linux WebKitGTK plays audio through GStreamer, and its codec support depends on the installed plugins. Native playback avoids that difference (Stack §H).

## Scope

1. Play audio natively in Rust with cpal or rodio. Do not play audio in the webview.
2. Hold both signals as 44.1 kHz buffers: the decoded input file and the restored output.
3. Switch between A and B at the same sample index in 50 ms or less.
4. Provide seek, play, and pause.
5. Add the compare player to a finished job in the queue and in the history.
6. Match the volume per SRS §10.4 and §10.5. Pick the EBU R128 crate (Stack §H). Show the gain offset in dB.
7. Read the input from its original path. When the file is missing or its SHA-256 differs from the history, show why the compare is unavailable (SRS §10.8).

## Out of scope

- A spectrogram view (backlog B-003)

## Test-first plan

1. Write Rust tests with a null audio sink. A switch keeps the sample index. A seek moves both buffers.
2. The integrated loudness of each fixture is within 0.1 LU of the conda ffmpeg `ebur128` filter. This threshold is inferred. Revise it with recorded evidence if the measurement shows that it is wrong.
3. With volume matching on, the louder version plays with the measured negative offset. The output file is unchanged.
4. A changed input file makes the compare unavailable with a reason.

Run the tests. They fail because the player does not exist.

## Acceptance criteria

- [ ] `Outcome` records a manual listening check on Windows and under WSLg.
- [ ] `Outcome` records the measured switch time.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core playback
pnpm verify
```
