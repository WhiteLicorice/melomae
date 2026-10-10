# 16 — Triage input files for restoration

**Status:** TODO
**Phase:** 2 — Metadata, triage, and provenance
**Depends on:** 03, 14
**SRS:** §4
**Stack:** §D, §G
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Tell the user which files can gain from restoration before any restore runs.

## Evidence

- Apollo was trained on simulated MP3 damage at 24 to 128 kb/s. This comes from a secondary source. Confirm it in the Apollo paper or repository and record the citation.
- A real 320 kb/s MP3 reached about 20.1 kHz in a rough spectrum check (`docs/evidence/2026-10-10-pivot.md`). Files like it are likely `low-benefit`.

## Scope

1. Walk the input files and folders (SRS §4.1).
2. For each file, read the container, codec, bitrate, sample rate, bit depth when known, and duration.
3. Measure the spectral cutoff on the decoded audio (task 03). Define the method in code comments and in `Outcome`.
4. Assign the verdicts in SRS §4.3.
5. Store the results in `triage_items` (task 14). On a rescan, read only files whose size or modification time changed. Remove rows for deleted files.
6. Never write to an input file.

## Calibration

1. Extend `tools/fixtures/make_fixtures.py` (dev-only, conda ffmpeg) to encode `C:\Lab\Apollo\asserts\input_wav.wav` as MP3 at 64, 128, 192, and 320 kb/s, as AAC at 128 and 256 kb/s, and as a FLAC transcoded from the 128 kb/s MP3.
2. Measure the cutoff of each fixture. Record the table in `Outcome`.
3. Set the high-bitrate threshold and the suspect-lossless threshold from that table. Record the values and the reason.

The thresholds are inferred until this calibration runs. If the measurements contradict the verdict classes in SRS §4.3, stop and report to the owner.

## Out of scope

- The triage screen (task 24)

## Test-first plan

1. Each calibration fixture gets its expected verdict. The FLAC transcoded from MP3 gets `suspect-lossless`.
2. A truncated file gets `unreadable` with a reason.
3. An incremental rescan after one file changes reads only that file.
4. A scan leaves every input file byte-identical.

Run the tests. They fail because the triage module does not exist.

## Acceptance criteria

- [ ] `Outcome` records the calibration table and the thresholds.
- [ ] `Outcome` records the scan time for 1,000 generated files.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core triage
pnpm verify
```
