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
5. Add the compare player to a finished job in the queue. Task 42 adds it to the history.
6. Match the volume per SRS §10.4 and §10.5. Pick the EBU R128 crate (Stack §H). Show the gain offset in dB.
7. Read the input from its original path. When the file is missing or its SHA-256 differs from the history, show why the compare is unavailable (SRS §10.8).
8. Add the volume-matching switch. It is on by default, and the user can turn it off (SRS §10.5). Read and store the value through the setting from task 19. Task 32 disables the switch in blind mode.
9. Show a loading state while both buffers decode and the loudness is measured. Show an error state with the file name and a retry when a decode fails.
10. Make the player work from the keyboard: Space plays and pauses, Left and Right seek, and one key selects each version. The position control has `role="slider"` and reports minutes and seconds.
11. State a maximum compare length or a memory budget. Two 4-minute stereo signals as 32-bit float at 44.1 kHz need about 169 MB. A 60-minute input needs about 2.5 GB. These numbers are computed from the sample format and not measured. A file over the limit shows a clear unavailable state. Streamed playback is a later task.
12. Show the label "Generated detail" next to the restored version, with the two-sentence explanation from section 9 of the proposal (SRS §1.4).
13. Do not build the measured cutoff view or a "Locate source" action. The MVP ruling cut both.

`tasks/ux/interface-proposal.md` section 3.4 proposes the layout and the keys. Read its MVP ruling first. The ruling wins over the section.

## Out of scope

- A spectrogram view (backlog B-003)

## Test-first plan

1. Write Rust tests with a null audio sink. A switch keeps the sample index. A seek moves both buffers.
2. The integrated loudness of each fixture is within 0.1 LU of the conda ffmpeg `ebur128` filter. This threshold is inferred. Revise it with recorded evidence if the measurement shows that it is wrong.
3. With volume matching on, the louder version plays with the measured negative offset. The output file is unchanged.
4. A changed input file makes the compare unavailable with a reason.
5. With volume matching off, both versions play with no offset. The stored value survives a restart.
6. A component test covers the loading state, the decode error state, and the over-limit state.
7. A component test drives play, seek, and the switch from the keyboard, and reads the slider value text.

Run the tests. They fail because the player does not exist.

## Acceptance criteria

- [ ] `Outcome` records a manual listening check on Windows and under WSLg.
- [ ] `Outcome` records the measured switch time.
- [ ] `Outcome` records the peak buffer memory for a 10-minute input, and the limit that the task chose.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core playback
pnpm verify
```
