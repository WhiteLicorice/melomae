# 45 — Measure and stop the MP3 trim to an estimated length

**Status:** DONE
**Phase:** 1 — Restoration engine
**Depends on:** 03
**SRS:** §6
**Stack:** §D
**Assigned to:** Agent
**Started:** 2026-11-18. Base `fd0606d` on `task/45`, tree clean. No gate had run on this branch.
**Outcome:** DONE 2026-11-18. Measured first, before any code change, each file against the ffmpeg oracle: headerless CBR 128 kb/s 266112 samples against 266112 (0 lost); headerless VBR `-q:a 4` 200448 against 266112 (65664 lost, 57 frames, 1.49 s); headerless VBR with a terminal APEv2 tag 201600 against 266112 (64512 lost); MP3 with a Xing header, the control, 264600 against 264600 (0). Red output: `.claygo/mt-evidence/reports/45-red.txt`. Cause, quoted from `symphonia-bundle-mp3-0.6.1/src/demuxer.rs`: with no Xing, Info or VBRI header the demuxer takes the `else` branch at `:459-472` and calls `estimate_num_mpeg_frames` (`:683-733`), which averages the first 16 frames or 16 KiB, `total_len as f64 / avg_mpeg_frame_len` at `:724-725`, then `track.with_num_frames(...)`; `:474-477` sets `Duration::from(num_frames)` because the timebase is one tick per sample. The VBR fixture's 731-byte first frame against 104 for the sustained tone's later frames is what puts the average 57 frames short. Melomae took that duration as `valid_frames` (`crates/melomae-core/src/decode/mod.rs:320-324`) and the decoder trimmed to it. Fix: `count_declared = format_info().short_name != "mp3" || mp3_declares_frame_count(path)` (`:320-330`) now gates both the trim bound and the completeness count, and `gapless(count_declared)` (`:506`, `:479-495`) turns off Symphonia's own trim to the same estimate for a headerless file; a headerless MP3 keeps its encoder delay, an MP3 with a header still trims its padding. Tests in `crates/melomae-core/tests/decode_formats.rs:319-390`: `headerless_cbr_mp3_decodes_every_frame_against_the_oracle`, `headerless_vbr_mp3_decodes_every_frame_against_the_oracle`, `headerless_vbr_mp3_with_a_terminal_apev2_tag_decodes_every_frame`, `mp3_with_a_xing_header_keeps_the_gapless_trim`; fixture `Fixtures::headerless_mp3` (`tests/setup/mod.rs:252`) and `write_apev2_tag` (`:674`). All results were rerun in `C:\mt\t45` alone after the shared build folder was withdrawn. Gate at `e6fbada`: `pnpm verify` exit 0, board 119 records, tasks unittest 12 passed, Vitest 1 passed, cargo test 274 passed and 4 ignored, pytest 45 passed. Every fixture now decodes to its oracle count exactly.

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

- [x] `Outcome` holds the measured frame counts for each fixture, before any change.
- [x] No headerless fixture loses more than one MP3 frame against the oracle.
- [x] Every task 03 fixture keeps its decoded count.
- [x] The new tests failed first for the expected reason, or `Outcome` says why not.
- [x] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core decode
pnpm verify
```
