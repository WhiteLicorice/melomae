# Fixtures

## `ori_wav.wav`

A 6-second, 44.1 kHz stereo 32-bit float audio clip. The task 16 calibration encodes it with the
pinned ffmpeg to measure the triage thresholds.

- Source: the upstream `JusperLee/Apollo` repository, first commit `d72b12c` (2024-09-07), at
  `asserts/48khz/sample1/ori_wav.wav`.
- The repository is licensed CC BY-SA 4.0.
- The separate rights in the recording are not confirmed.
- The expected SHA-256 is in `ori_wav.sha256`.

The file is stored through Git LFS. Run `git lfs pull` when it is missing or is a pointer file.
The calibration test fails with that command when the audio is absent.
