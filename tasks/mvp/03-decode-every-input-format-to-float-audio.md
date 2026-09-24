# 03 — Decode every input format to 44.1 kHz float audio

**Status:** TODO
**Phase:** 1 — Restoration engine
**Depends on:** 00
**SRS:** §5, §6, §9
**Stack:** §D
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Decode MP3, AAC/M4A, Opus (WebM and Ogg), Vorbis, FLAC, ALAC, WAV, and AIFF to planar 32-bit float at 44.1 kHz in `melomae-core`.

## Evidence

- Symphonia has no Opus decoder. Its Opus work was an open WIP pull request (#398) on 2026-09-24.
- Opus is the primary input. YouTube's best audio stream is Opus (format 251).
- Opus always decodes at 48 kHz. Apollo needs 44.1 kHz.
- The owner's old pipeline resampled with ffmpeg swr at `filter_size=256:phase_shift=24:cutoff=0.98` (`C:\Lab\Apollo\inference2.py:39`).
- The owner's library holds names such as `Florida!!!.opus` and `Habits (Stay High).opus`, which broke an old batch script (`C:\Lab\Apollo\run_batch.py` docstring).

## Scope

1. Demux WebM/MKV, Ogg, MP4, WAV, and AIFF with symphonia.
2. Decode Opus packets with libopus, the reference decoder. Use a crate that builds libopus from vendored source on MSVC and Linux, with no system package. Confirm the current options at execution time. Do not use a partial or experimental Opus decoder.
3. Decode all other codecs with symphonia.
4. Honor Opus pre-skip, and Matroska `CodecDelay` and `DiscardPadding`, so the decoded length is exact.
5. Resample to 44.1 kHz with rubato's high-quality sinc resampler. Skip the resample when the input is already 44.1 kHz.
6. Keep the native channel count.
7. Return a typed error that names the file and the reason for an undecodable input.
8. Add `tools/fixtures/make_fixtures.py`. It is dev-only and uses the conda ffmpeg. It builds the test fixtures from `C:\Lab\Apollo\asserts\input_wav.wav` (CC BY-SA 4.0). Record the attribution next to the fixtures. Never commit downloaded audio.

## Out of scope

- Streaming decode for very long files (backlog B-005)
- Tag reading (tasks 15 and 24)

## Test-first plan

1. For each fixture, the decoded length matches the ffmpeg reference within 1 sample. The Opus tolerance is 1 ms.
2. The 48 kHz decode of the Opus fixture matches the libopus reference output exactly. Use `ffmpeg -c:a libopus -i …` from the conda build as a dev-only oracle. First confirm that the conda build has the libopus decoder. If it does not, use `opusdec`.
3. A resampled sweep scores SNR ≥ 80 dB against the ffmpeg swr reference with the settings above.
4. A resampled 1 kHz tone gives THD+N ≤ −100 dB.
5. Files named `Florida!!!.opus`, `Habits (Stay High).opus`, and `Sigur Rós.flac` decode.

The thresholds in items 3 and 4 are inferred. If the measurement shows they are wrong, revise them with the recorded evidence. Do not loosen them silently.

Run the tests. They fail because the decoder does not exist.

## Acceptance criteria

- [ ] Every listed format decodes with no ffmpeg on PATH.
- [ ] Opus decode matches the libopus reference exactly.
- [ ] The resampler numbers are recorded in `Outcome`.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core decode
pnpm verify
```
