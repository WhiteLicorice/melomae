# 06 — Encode FLAC, WAV, and Opus output

**Status:** DONE
**Phase:** 1 — Restoration engine
**Depends on:** 03
**SRS:** §7
**Stack:** §D
**Assigned to:** Agent
**Started:** 0049784, clean tree. Baseline gate at 0049784: board 53 records, tasks unittest 9 passed, Vitest 1 passed, cargo test 98 passed, pytest 13 passed.
**Outcome:** DONE. `melomae_core::encode::encode` writes one `AudioBuffer` to a path as FLAC 24-bit, WAV IEEE float 32-bit, or Opus in Ogg. FLAC uses the pure-Rust `flacenc` 0.5.1 (Apache-2.0); its `constant::MAX_BITS_PER_SAMPLE` is 24, so the 24-bit depth needs no libFLAC. WAV is a hand-written RIFF/WAVE with format tag 3. Opus resamples with the existing `AudioBuffer::resample_to` (rubato 5.0.1, MIT), encodes with `opusic-c` 1.6.1 / libopus 1.6.1 (BSD-3-Clause) at 256 kb/s VBR, and muxes hand-written Ogg pages. Every direct and transitive license is permissive and compatible with GPL-3.0-or-later. FLAC rounds to the nearest 24-bit value with no dither, clamps samples outside ±1.0 to the rails, and returns the clamp count. WAV and Opus clamp nothing and report a count of zero. Each writer writes a hidden temporary file beside the target and renames it into place only when the stream is complete. Measured: the FLAC round trip through `decode_file` equals the 24-bit-quantized input exactly; the WAV round trip is bit-exact, including samples above 1.0; the Opus output decodes, its length is within 1 ms, and its aligned correlation is at least 0.95; the overflowing fixture reports 3 clamps; the pinned ffmpeg opens all three outputs and decodes samples from each. One `flacenc` defect is worked around: it lowers the STREAMINFO minimum block size to the last, short frame while it emits a fixed-strategy stream, so symphonia reads the stream as variable-block and fails its probe. The encoder restores min == max block size before it writes. Tests: 11 unit and 6 integration (`cargo test -p melomae-core encode`). Gate `pnpm verify` exit 0 (`SCRATCH/evidence-06/verify.txt`, ending `EXIT=0`): board 53 records valid, tasks unittest 9 passed, Vitest 1 passed, cargo test 115 passed, pytest 13 passed. The cargo total is 17 above the 98 baseline. Fix round 1 (2026-10-11) resolved B1, N1, and N2 with one input check at the public `encode` entry, before the output directory or the temporary file exists: it rejects a non-finite sample (naming the channel and the frame index), a sample rate that is not 44100 Hz, and an empty buffer. Finite overflow is unchanged. Six integration tests and one unit test cover the check. They failed first (`SCRATCH/evidence-06/red-round1.txt`; every case returned `EncodeOutcome` success), and pass now (`SCRATCH/evidence-06/green-round1.txt`). Round-1 gate `pnpm verify` exit 0 (`SCRATCH/evidence-06/verify-round1.txt`, ending `EXIT=0`): board 53 records valid, tasks unittest 9 passed, Vitest 1 passed, cargo test 122 passed, pytest 13 passed.

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

- [x] All three formats open in the task 03 decoder and in the conda ffmpeg (a dev check). The dev check now uses the pinned ffmpeg from `tools/fetch_ffmpeg.py`.
- [x] The new tests failed first for the expected reason (`SCRATCH/evidence-06/red.txt`, `error[E0432]: unresolved import melomae_core::encode`).
- [x] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core encode
pnpm verify
```
