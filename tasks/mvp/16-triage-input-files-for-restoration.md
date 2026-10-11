# 16 — Triage input files for restoration

**Status:** DONE
**Phase:** 2 — Metadata, triage, and provenance
**Depends on:** 03, 14
**SRS:** §4
**Stack:** §D, §G
**Assigned to:** Agent
**Started:** 0049784, clean tree. Baseline gate at 0049784: board 53 records, tasks unittest 9 passed, Vitest 1 passed, cargo test 98 passed, pytest 13 passed.
**Outcome:** DONE. A new `triage` module walks files and folders, measures the spectral cutoff on the task 03 decode, assigns the SRS §4.3 verdicts, and stores rows in `triage_items`. A rescan reads only the files whose size or modification time changed and removes the rows of deleted files. Triage opens every input read-only and never writes one. The thresholds come from the calibration on `fixtures/ori_wav.wav`, committed through Git LFS: the cutoff is 80 dB below the spectrum peak, and the high-bitrate and suspect-lossless lines are both 19500 Hz. Measured verdicts: the source WAV and the lossless FLAC are `lossless` at 22050 Hz, MP3 64/128/192 are `restore` at 11289/16349/18605 Hz, MP3 320 is `low-benefit` at 20015 Hz, AAC 128/256 are `low-benefit` at 19999/22050 Hz, and the FLAC from the 128 kb/s MP3 is `suspect-lossless` at 16349 Hz. A truncated MP3 is `unreadable` with a reason. The 1,000-file scan took 8.82 s, about 113 files per second. `cargo test -p melomae-core` passes 115 tests, and clippy and fmt are clean. The gate `pnpm verify` exits 0: board 53 records, tasks unittest 9 passed, Vitest 1 passed, cargo test 115 passed, pytest 13 passed. Fix round 1 (review findings B1 to B4) samples positions over the whole file and every channel, stores nanosecond modification times under schema version 2, and reports float PCM and ALAC bit depth. The round-1 gate at `42efe75` exits 0 with 122 cargo tests. Fix round 2 selects the first, central, and last valid start for every file, including short ones. The round-2 gate at `556078d` exits 0 with 125 cargo tests. See the detail below.

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

**Changed by the author on 2026-10-11.** `make_fixtures.py` and the conda ffmpeg are gone. The calibration source is `fixtures/ori_wav.wav`, committed through Git LFS. The fixture builders are in `crates/melomae-core/tests/setup/mod.rs`. The full-band source replaced `C:\Lab\Apollo\asserts\input_wav.wav`, which is band-limited near 4.5 kHz and cannot show a codec cutoff. `Outcome detail` records the three measured tables.

## Out of scope

- The triage screen (task 24)

## Test-first plan

1. Each calibration fixture gets its expected verdict. The FLAC transcoded from MP3 gets `suspect-lossless`.
2. A truncated file gets `unreadable` with a reason.
3. An incremental rescan after one file changes reads only that file.
4. A scan leaves every input file byte-identical.

Run the tests. They fail because the triage module does not exist.

## Acceptance criteria

- [x] `Outcome` records the calibration table and the thresholds.
- [x] `Outcome` records the scan time for 1,000 generated files.
- [x] The new tests failed first for the expected reason.
- [x] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core triage
pnpm verify
```

## Outcome detail

### The cutoff method

Triage decodes every file with `melomae_core::decode::decode_file`, then measures the spectral cutoff
of the decoded 44.1 kHz audio. The method is Welch's method. The spectrum is averaged over
Hann-windowed frames of 8192 samples. The frames are taken at up to 128 start positions spread
evenly over the whole file, from the first valid start to the last, and every channel is analyzed at
every selected position. A silent or narrow-band opening therefore cannot hide the rest of the file,
and a silent channel cannot hide the others. The average is smoothed by a moving average that spans
five bins. The peak bin is the reference. The cutoff is the highest frequency that is the top of a
run of bins within 80 dB of the peak. The run is at least 200 Hz wide, so one spike does not count.
The code and the comments are in `crates/melomae-core/src/triage/cutoff.rs`.

### Table 1. The first source, rejected

`C:\Lab\Apollo\asserts\input_wav.wav` is band-limited near 4.5 kHz. An independent numpy spectrum
reads -34 dB at 4 kHz, -113 dB at 5 kHz, and a floor near -125 dB above. The source has no high
frequency for a codec to remove. Cutoff in Hz:

| Fixture | 60 dB | 80 dB | 100 dB | 120 dB |
|---|---|---|---|---|
| MP3 64 | 4339 | 4457 | 4732 | 9921 |
| MP3 128 | 4344 | 4474 | 4737 | 15638 |
| MP3 192 | 4344 | 4479 | 4743 | 18244 |
| MP3 320 | 4344 | 4479 | 4743 | 19762 |
| AAC 128 | 4344 | 4463 | 7865 | 19267 |
| AAC 256 | 4344 | 4479 | 6158 | 20650 |
| FLAC from MP3 128 | 4344 | 4474 | 4737 | 15638 |
| FLAC lossless | 4344 | 4479 | 4743 | 9921 |

No threshold separates the classes. The lossless FLAC reads like a 64 kb/s MP3. The author approved a
full-band source on this evidence.

### Table 2. The synthetic source, rejected

A pink-noise, tone, and sweep source built in test setup. The MP3 classes separate. The ffmpeg native
AAC 128 measures above MP3 320 at every threshold. Cutoff in Hz:

| Fixture | 40 dB | 60 dB | 80 dB | 100 dB | 120 dB |
|---|---|---|---|---|---|
| source WAV | 22050 | 22050 | 22050 | 22050 | 22050 |
| MP3 64 | 11030 | 11197 | 11321 | 11558 | 11908 |
| MP3 128 | 16548 | 16694 | 16823 | 17027 | 17377 |
| MP3 192 | 18621 | 18745 | 18901 | 19138 | 19482 |
| MP3 320 | 20020 | 20150 | 20274 | 20473 | 20850 |
| AAC 128 | 19983 | 20031 | 22050 | 22050 | 22050 |
| AAC 256 | 22050 | 22050 | 22050 | 22050 | 22050 |
| FLAC from MP3 128 | 16548 | 16694 | 16823 | 17027 | 17377 |
| FLAC lossless | 22050 | 22050 | 22050 | 22050 | 22050 |

### Table 3. `fixtures/ori_wav.wav`, accepted

Real full-band audio, committed through Git LFS. A threshold below 80 dB tracks the source rolloff,
and no codec shows. At 80 dB and above, each lossy low-pass reads its own corner and the lossless
control reaches Nyquist. Cutoff in Hz:

| Fixture | 40 dB | 60 dB | 80 dB | 100 dB | 120 dB | 140 dB |
|---|---|---|---|---|---|---|
| source WAV | 11375 | 16414 | 22050 | 22050 | 22050 | 22050 |
| MP3 64 | 10621 | 11160 | 11289 | 11472 | 11919 | 22050 |
| MP3 128 | 11230 | 15988 | 16349 | 16688 | 16839 | 22050 |
| MP3 192 | 11262 | 16015 | 18605 | 18782 | 18933 | 22050 |
| MP3 320 | 11262 | 16349 | 20015 | 20166 | 20284 | 22050 |
| AAC 128 | 11181 | 16430 | 19999 | 21878 | 22050 | 22050 |
| AAC 256 | 11262 | 16414 | 22050 | 22050 | 22050 | 22050 |
| FLAC from MP3 128 | 11230 | 15988 | 16349 | 16688 | 16839 | 22050 |
| FLAC lossless | 11375 | 16414 | 22050 | 22050 | 22050 | 22050 |

### The thresholds

`DROP_DB = 80` in `cutoff.rs`. `HIGH_BITRATE_CUTOFF_HZ = 19500` and `SUSPECT_LOSSLESS_CUTOFF_HZ =
19500` in `triage/mod.rs`. The high-bitrate line sits between MP3 192 (18605 Hz) and the high-cutoff
lossy files (MP3 320 at 20015 Hz). The suspect-lossless line sits between the FLAC from the MP3 128
(16349 Hz) and the full-band source (22050 Hz). One threshold applies to every file. It is not tuned
to one file.

### The AAC cause (inferred)

The pinned ffmpeg has only the native `aac` encoder (`aac` and `aac_mf`, no libfdk). The native
encoder keeps content near 20 kHz at 128 kb/s, so AAC 128 measures 19999 Hz and AAC 256 measures
22050 Hz, both at or above the high-bitrate line. The author ruled that the verdict is advice and the
user can restore any file (SRS §4.5), so both keep `low-benefit`. Inferred, not confirmed: a
band-limiting encoder such as libfdk would put a 128 kb/s AAC near 16 kHz and give `restore`.

### The Apollo training claim

Confirmed. `C:\Lab\Apollo\README.md` line 64 and `C:\Lab\Apollo\Apollo\README.md` line 68 record:
"We simulated various bitrate scenarios by applying MP3 codecs with bitrates of [24000, 32000, 48000,
64000, 96000, 128000]." `look2hear/datas/musdb_moisesdb_datamodule.py` line 35 uses
`random.choice([24000, 32000, 48000, 64000, 96000, 128000])`, and line 37 sets `format: mp3`.
`configs/apollo.yaml` lines 11 to 14 set `codec_type: mp3`. The range is 24 to 128 kb/s. The paper is
Li, Kai and Luo, Yi, "Apollo: Band-sequence Modeling for High-Quality Music Restoration in
Compressed Audio", ICASSP 2025.

### The fixtures and the tests

`fixtures/ori_wav.wav` is 6 s, 44.1 kHz, stereo, 32-bit float. It is stored through Git LFS, with
`fixtures/ori_wav.sha256` and one `.gitattributes` pattern. `fixtures/README.md` records the source
and the licence. The calibration test fails with `Run: git lfs pull` when the file is missing or is a
pointer, and it checks the SHA-256. CI pulls the LFS objects behind an `actions/cache@v4` keyed on
the OS and the `git lfs ls-files --long` output.

The tests: eleven unit tests in `src/triage` (five cutoff, six triage), the calibration test and the
truncated-file test in `tests/triage_calibration.rs`, the scan, rescan, removal, never-writes,
`e2e_triage_scan_a_folder_into_the_store`, and 1,000-file tests in `tests/triage_scan.rs`, the six
round-1 fix tests in `tests/triage_round1_fixes.rs`, and the two round-2 fix tests in
`tests/triage_round2_fixes.rs`.

### The files

`crates/melomae-core/src/triage/mod.rs` holds the verdicts, the scan, and the store writes.
`crates/melomae-core/src/triage/cutoff.rs` holds the measurement.
`crates/melomae-core/src/store/mod.rs` gains `triage_items` and `delete_triage_item`.
`crates/melomae-core/tests/setup/mod.rs` gains the calibration builders and the LFS guard.
`Cargo.toml` gains `realfft` and the `sha2` dev-dependency. `.github/workflows/verify.yml` gains the
LFS cache steps.

### Fix round 1 (review findings B1 to B4)

The review failed the task with four blocking findings. The fixes are:

- B1 and B2. The window selection was a prefix, and the first channel could use the whole budget.
  `averaged_spectrum` now takes up to 128 start positions spread evenly over the whole file and
  analyzes every channel at every position. The position count is the budget.
- B3. `modified_at` was whole seconds, so a fast same-size edit could be skipped. It is now
  nanoseconds since the Unix epoch. Store migration 2 deletes every cached `triage_items` row, so a
  whole-second value cannot match. `scanned_at` stays in seconds.
- B4. A float PCM WAV reported no depth. The depth now comes from the codec id (32 for F32, 64 for
  F64), and ALAC's depth comes from byte 5 of its magic cookie. A lossy codec reports no depth.

Record fixes for review items 2 and 4: the bitrate comment now says the average file rate includes
tags and cover art, and the smoothing and threshold-direction comments are corrected. The smoothing
behavior did not change.

The re-run calibration at the fix head. Cutoff in Hz, at 80 dB:

| Fixture | Before | After | Verdict |
|---|---|---|---|
| source WAV | 22050 | 22050 | lossless |
| MP3 64 | 11289 | 11294 | restore |
| MP3 128 | 16349 | 16349 | restore |
| MP3 192 | 18605 | 18605 | restore |
| MP3 320 | 20015 | 20020 | low-benefit |
| AAC 128 | 19999 | 19999 | low-benefit |
| AAC 256 | 22050 | 22050 | low-benefit |
| FLAC from MP3 128 | 16349 | 16349 | suspect-lossless |
| FLAC lossless | 22050 | 22050 | lossless |

Every verdict holds. The two moved cutoffs moved by one bin (5.38 Hz). The source WAV now also
reports bit depth 32.

Red first: `SCRATCH/evidence-16/red-round1-triage.txt` and `red-round1-store.txt` show six new
integration tests, one new migration test, and one changed schema assertion failing on the pre-fix
code for the expected reasons. Green: `green-round1.txt`.

The round-1 gate at `42efe75` exits 0: board 53 records, tasks unittest 9 passed, Vitest 1 passed,
cargo test 122 passed, pytest 13 passed.

### Fix round 2 (review finding B1b)

The re-review found one blocking finding. The start selection fell to one position, start 0, when the
span was below the hop, so the final frames of a short file were never measured. A short WAV with a
silent opening got `suspect-lossless` at cutoff 0.

- The selection is now `analysis_starts`. Whenever the span is nonzero it always includes the first
  valid start (0), the last valid start (`span`), and the central start `span / 2`, plus an evenly
  spread grid up to `MAX_POSITIONS`. Duplicate starts are removed and the result is sorted. A zero
  span still uses one start, at 0. Every channel is analyzed at every selected start.
- The central start is `span / 2`, the midpoint between the first and the last valid start.

The re-run calibration at the fix head is unchanged: every verdict holds and no cutoff moved. The
source WAV remains `lossless` at 22050 Hz, MP3 64/128/192 remain `restore` at 11294/16349/18605 Hz,
MP3 320 remains `low-benefit` at 20020 Hz, AAC 128/256 remain `low-benefit` at 19999/22050 Hz, and
the FLAC from the 128 kb/s MP3 remains `suspect-lossless` at 16349 Hz.

Red first: `SCRATCH/evidence-16/red-round2.txt` shows the two short-file regressions failing at
runtime with `suspect-lossless` at cutoff 0, and the selection unit test failing to compile because
`analysis_starts` did not exist. Green: `green-round2.txt`.

The round-2 gate at `556078d` exits 0: board 53 records, tasks unittest 9 passed, Vitest 1 passed,
cargo test 125 passed, pytest 13 passed.
