# B-065 — Treat mu-law and A-law WAV files as lossy

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 16

## Impact

A mu-law or A-law WAV file is companded, so it is lossy. Triage counts it as lossless PCM. The file then gets the verdict `suspect-lossless` and a bit depth, and SRS §4.3 expects `restore`.

## Evidence

The ponytail audit of 2026-11-11 read the code at `master` `23f0bcf`. It built and ran nothing. The orchestrator read each cited line again on the same day.

- `crates/melomae-core/src/triage/mod.rs:302-303`: `is_pcm` accepts every codec from `CODEC_ID_PCM_S32LE` to `CODEC_ID_PCM_MULAW`.
- The audit cites `symphonia-core-0.6.1/src/codecs/audio.rs:380-382`: A-law and mu-law are the last two codes of that range. The orchestrator did not open that file.
- Nobody ran a mu-law file through triage.

## Why this does not block MVP

A music library rarely holds a companded WAV file. The user can still select the file by hand.

## Expected behavior

The PCM range ends before A-law and mu-law. Triage counts both as lossy codecs.

## Promotion condition

Promote this task when a user reports a wrong verdict for such a file, or with the next triage change.

## Acceptance criteria

- [ ] A mu-law WAV fixture gets `restore`.
- [ ] Every task 16 calibration verdict stays the same.
- [ ] A test fails before the change and passes after it.
