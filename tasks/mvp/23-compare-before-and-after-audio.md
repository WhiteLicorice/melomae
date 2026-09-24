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
2. Hold both signals as 44.1 kHz buffers: the decoded source and the restored output.
3. Switch between A and B at the same sample index in 50 ms or less.
4. Provide seek, play, and pause.
5. Add the compare player to a finished job in the queue and the library.

## Out of scope

- A spectrogram view (backlog B-003)

## Test-first plan

Write Rust tests with a null audio sink. A switch keeps the sample index. A seek moves both buffers.

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
