# Test fixtures

`make_fixtures.py` builds the decode test inputs and the ffmpeg reference decodes that
`crates/melomae-core` checks against. It is dev-only. The shipped app never uses ffmpeg.

Run it with the conda ffmpeg:

```powershell
python tools/fixtures/make_fixtures.py
```

## What it writes

It reads the read-only Apollo asset `C:\Lab\Apollo\asserts\input_wav.wav` and writes into
`crates/melomae-core/tests/fixtures`:

- one encoded input per supported format: `wav_44100.wav`, `aiff_44100.aiff`, `flac_44100.flac`,
  `mp3_44100.mp3`, `aac_44100.m4a`, `alac_44100.m4a`, `vorbis_44100.ogg`, `opus_48000.opus`,
  and `opus_48000.webm`;
- a raw little-endian `f32` reference decode next to each input, named `<input>.ffmpeg.f32`,
  decoded at the input's native rate. The Opus references are decoded with `-c:a libopus`, the
  reference decoder;
- `sweep_48000.wav`, a synthesized logarithmic sweep, and `sweep_44100.swr.f32`, the ffmpeg swr
  resample of it with the owner's old settings
  (`filter_size=256:phase_shift=24:cutoff=0.98`, `C:\Lab\Apollo\inference2.py:39`);
- the awkward file names `Florida!!!.opus`, `Habits (Stay High).opus`, and `Sigur Rós.flac`,
  which re-use the Opus and FLAC encodes under names that broke the owner's old batch script;
- `ATTRIBUTION.md`, next to the fixtures.

## Not committed

`crates/melomae-core/tests/fixtures` is gitignored. The files are large, and the encoded inputs
and references are derived from the read-only Apollo source. Commit the script, not its output.

The Rust tests read whatever the script produced. A case whose fixture is absent is skipped with
a message, so a machine without ffmpeg still builds and tests.

## Attribution

`C:\Lab\Apollo\asserts\input_wav.wav` is part of the Apollo project
(<https://github.com/JusperLee/Apollo>) and is licensed CC BY-SA 4.0. Apollo: Li, Kai and
Luo, Yi, "Apollo: Band-sequence Modeling for High-Quality Music Restoration in Compressed
Audio", ICASSP 2025. The sweep fixture is synthesized by the script and carries no third-party
rights.
