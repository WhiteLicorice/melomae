# 06 — Encode FLAC, WAV, and Opus output

**Status:** TODO
**Phase:** 1 — Restoration engine
**Depends on:** 03
**SRS:** §7
**Stack:** §D
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Write restored audio as FLAC 24-bit (the default), WAV 32-bit float, or Opus in Ogg.

## Evidence

- The pitch lists FLAC, WAV, and Opus output (`docs/source/pitch.md`).
- The owner's old pipeline wrote 32-bit float WAV to avoid quantization (`C:\Lab\Apollo\inference2.py:77-85`).
- Apollo output can exceed ±1.0. Nothing in the model bounds it.

## Scope

1. FLAC: 44.1 kHz, 24-bit, rounded, no dither. Confirm that a pure-Rust encoder supports 24-bit. If none does, use libFLAC.
2. WAV: 44.1 kHz, IEEE float 32-bit.
3. Opus: resample 44.1 kHz to 48 kHz, 256 kb/s VBR, in Ogg with a correct `OpusHead` pre-skip. Use libopus.
4. For integer formats, clamp samples outside ±1.0. Return the clamp count with the result.
5. Write to a temporary file. Rename it only when it is complete.

## Out of scope

- Tags and cover art (task 15)

## Test-first plan

1. A FLAC round trip through the task 03 decoder equals the 24-bit-quantized input exactly.
2. A WAV round trip is bit-exact.
3. An Opus output decodes, its length matches within 1 ms, and its aligned correlation with the input is at least 0.95.
4. An input with samples above 1.0 reports the correct clamp count.

Run the tests. They fail because the encoders do not exist.

## Acceptance criteria

- [ ] All three formats open in the task 03 decoder and in the conda ffmpeg (a dev check).
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core encode
pnpm verify
```
