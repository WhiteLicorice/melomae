# 03 — Decode every input format to 44.1 kHz float audio

**Status:** DONE
**Phase:** 1 — Restoration engine
**Depends on:** 00
**SRS:** §4, §6, §9
**Stack:** §D
**Assigned to:** Agent
**Started:** fd62a4e, clean tree. Baseline gate at fd62a4e: board 45 records, unittest 9 passed, Vitest 1 passed, cargo test 1 passed, pytest 8 passed.
**Outcome:** DONE. `melomae_core::decode` decodes MP3, AAC/M4A, Opus (Ogg and WebM), Vorbis, FLAC, ALAC, WAV, and AIFF to planar f32 at 44.1 kHz. Crates: symphonia 0.6.1 (demux and non-Opus codecs), opusic-c 1.6.1 / opusic-sys 0.7.5 (libopus 1.6.1, vendored source), rubato 5.0.1. Measured: every fixture length equals the ffmpeg reference (1 sample, Opus 1 ms); the 48 kHz Opus decode sits within max |diff| 1.5e-5 of the conda libopus 1.6.1 reference (tolerance 1e-4); the resampled 1 kHz tone gives THD+N −136.0 dB; the analytic 20 Hz–20 kHz sweep scores SNR 105.8 dB; the ffmpeg swr comparison reads 105.7 dB and is recorded, not gated. Fix round 1 resolved the eight review findings (F1–F8), including F7 (mapped multichannel Opus) and F8 (a 20 kHz gate). Fix round 2 resolved the five blocking findings (B1–B5) and three required gaps (C1–C3): a truncated stream is a typed error unless a Matroska structural scan proves a valid end; the `OpusHead` output gain is applied once; the MP4 edit list is selected by `tkhd` and a repeated, discontinuous, or non-unit-rate sequence is a typed `UnsupportedFormat` error; the Matroska scanner is bounded to the physical file and the nearest finite ancestor with a block-table budget; and the Opus mapping families are scoped with a version check. Supported Opus mappings: family 0 (unpacked mono and stereo) and family 1 up to eight channels whose stream and coupled counts sum to no more than the channel count; other legal headers return `UnsupportedFormat` (round 3 brought family 255 out of scope in every container). `cargo test -p melomae-core` passes 55 lib, 4 e2e, 9 format, 1 dependency, 8 round-1, 2 round-2, and 8 store tests; `cargo clippy -p melomae-core --all-targets -- -D warnings` and `cargo fmt --all -- --check` are clean. These are local counts. Fix-round-2 gate `pnpm verify` exit 0 (`SCRATCH/evidence-03/verify-round2.txt`, ending `EXIT=0`): fetch_ffmpeg ok; board 47 records valid; Python unittest 9 passed; tsc and eslint ok; Vitest 1 passed; vite build ok; `cargo fmt --check` clean; `cargo clippy --workspace -- -D warnings` clean; `cargo test --workspace` 87 passed, 0 failed (55 lib, 4 e2e, 9 format, 1 dependency, 8 round-1, 2 round-2, 1 store e2e, 2 store recovery, 5 store regressions); pytest 13 passed, 1 pre-existing FutureWarning. Fix round 3 (2026-10-11) resolved the four blocking findings (D1 to D4). D1: the Matroska scanner passes the nearest finite ancestor end into every master parser, separately from the master's own optional end, so an unknown-size Cluster inside a finite Segment ends at the Segment end at the latest; it rejects a header or body that crosses that ancestor, keeps scanning top-level elements to the physical end, and sets `valid_eof` only after reaching the file end at an element boundary with no unread byte. D3: box traversal and `moov` reading return `Result`, `Ok(None)` means only "not present", and a box whose header or body crosses its parent or the physical end is `Malformed`; the errors propagate through the track lookup and the decode. D4: Opus mapping family 255 is out of scope in every container and returns `UnsupportedFormat` naming the family and the channel count; for Ogg, when the probe rejects the header, the reader reads the `OpusHead` from the first Ogg page to produce that error (Symphonia and opusic-c are not patched). D2: when the container states a frame count and the decoded total is lower, the decoder returns a typed `Malformed` error naming the file, the declared count, and the decoded count; the rule applies only when the track timebase is one tick per sample, and a file with no declared count keeps today's behavior (round 4 corrected this: Symphonia's `num_frames` for an MP3 is a bitrate estimate when no Xing/Info/VBRI header exists, so it is not used; an MP3 with no header remains a backlog ticket). Measured declared counts (samples): WAV/AIFF/FLAC/MP3/ALAC/Vorbis 264600, AAC/M4A 266240 (the track timespan, priming included; the edit list still trims the output); Vorbis sets none when cut; Opus and Matroska are outside this path. Local: `cargo test -p melomae-core` exit 0 (57 lib, 4 e2e, 9 format, 1 dependency, 8 round-1, 2 round-2, 3 round-3, 8 store), `cargo clippy -p melomae-core --all-targets -- -D warnings` and `cargo fmt --all -- --check` clean. Fix-round-3 gate `pnpm verify` exit 0 (`SCRATCH/evidence-03/verify-round3.txt`, ending `EXIT=0`): fetch_ffmpeg ok; board 47 records valid; Python unittest 9 passed; tsc and eslint ok; Vitest 1 passed; vite build ok; `cargo fmt --check` clean; `cargo clippy --workspace -- -D warnings` clean; `cargo test --workspace` 92 passed, 0 failed (57 lib, 4 e2e, 9 format, 1 dependency, 8 round-1, 2 round-2, 3 round-3, 1 store e2e, 2 store recovery, 5 store regressions); pytest 13 passed, 1 pre-existing FutureWarning. Fix round 4 (2026-10-11) resolved two blocking findings (E1, E2) and one non-blocking comment (N1). E1 corrected round-3 ruling 2: Symphonia's `num_frames` is not always a declaration, so the decoder now compares the decoded total only when the first MPEG frame actually carries a Xing/Info frame-count field or a VBRI header (a bounded 8 KiB read of the first frame); an intact headerless MP3 with a terminal APEv2 tag decodes, and the declared-count MP3 cuts still return `Malformed`. E2: every Matroska element ID and size-vint read is bounded by the nearest finite limit, a body that starts past that limit is a truncation even when its size is unknown, a position past a limit is an error with only equality completing the boundary, and `valid_eof` requires exact completion. N1: the laced-block test comment now names EBML lacing. Local round-4: `cargo test -p melomae-core` exit 0 (60 lib, 4 e2e, 9 format, 1 dependency, 8 round-1, 2 round-2, 3 round-3, 2 round-4, 8 store), `cargo clippy -p melomae-core --all-targets -- -D warnings` and `cargo fmt --all -- --check` clean. Fix-round-4 gate `pnpm verify` exit 0 (`SCRATCH/evidence-03/verify-round4.txt`, ending `EXIT=0`): fetch_ffmpeg ok; board 47 records valid; Python unittest 9 passed; tsc and eslint ok; Vitest 1 passed; vite build ok; `cargo fmt --check` clean; `cargo clippy --workspace -- -D warnings` clean; `cargo test --workspace` 97 passed, 0 failed (60 lib, 4 e2e, 9 format, 1 dependency, 8 round-1, 2 round-2, 3 round-3, 2 round-4, 1 store e2e, 2 store recovery, 5 store regressions); pytest 13 passed, 1 pre-existing FutureWarning. Fix round 5 (2026-10-11) resolved one blocking finding (NEW1): the MP3 declared-count detector now skips leading ID3v2 tags by reading only their 10-byte headers (validating the synchsafe size and adding 10 for a footer flag) and seeking past each one, repeating for consecutive tags, so a tag body is never read or allocated; it then reads a bounded 8 KiB window at the audio offset to find the first MPEG frame. A tag larger than the old fixed prefix no longer hides the frame count, so a cut behind a large tag returns `Malformed`. When no MPEG frame is found in that window the count is treated as undeclared. Local round-5: `cargo test -p melomae-core` exit 0 (60 lib, 4 e2e, 9 format, 1 dependency, 8 round-1, 2 round-2, 3 round-3, 2 round-4, 1 round-5, 8 store), `cargo clippy -p melomae-core --all-targets -- -D warnings` and `cargo fmt --all -- --check` clean. Fix-round-5 gate `pnpm verify` exit 0 (`SCRATCH/evidence-03/verify-round5.txt`, ending `EXIT=0`): fetch_ffmpeg ok; board 47 records valid; Python unittest 9 passed; tsc and eslint ok; Vitest 1 passed; vite build ok; `cargo fmt --check` clean; `cargo clippy --workspace -- -D warnings` clean; `cargo test --workspace` 98 passed, 0 failed (60 lib, 4 e2e, 9 format, 1 dependency, 8 round-1, 2 round-2, 3 round-3, 2 round-4, 1 round-5, 1 store e2e, 2 store recovery, 5 store regressions); pytest 13 passed, 1 pre-existing FutureWarning. See Outcome detail.

## Goal

Decode MP3, AAC/M4A, Opus (WebM and Ogg), Vorbis, FLAC, ALAC, WAV, and AIFF to planar 32-bit float at 44.1 kHz in `melomae-core`.

## Evidence

- Symphonia has no Opus decoder. Its Opus work was an open WIP pull request (#398) on 2026-09-24.
- Users own Opus files in Ogg and WebM containers, so Opus decode stays in scope.
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

   **Changed by the author on 2026-10-10.** The author's words: "Just recreate the fixtures during the test and delete them afterward, standard setup-and-teardown." This replaced the local-only fixtures and the skip-when-absent behavior. The tests now synthesize the source in Rust, encode it with a pinned ffmpeg (`tools/fetch_ffmpeg.py`, a BtbN monthly archive), and delete everything when the test ends. `make_fixtures.py` and its README are deleted, and no CC BY-SA attribution is needed because the source is synthesized.

## Out of scope

- Streaming decode for very long files (backlog B-005)
- Tag reading (task 15)

## Test-first plan

1. For each fixture, the decoded length matches the ffmpeg reference within 1 sample. The Opus tolerance is 1 ms.
2. The 48 kHz decode of the Opus fixture matches the libopus reference output exactly. Use `ffmpeg -c:a libopus -i …` from the conda build as a dev-only oracle. First confirm that the conda build has the libopus decoder. If it does not, use `opusdec`.
3. A resampled sweep scores SNR ≥ 80 dB against the ffmpeg swr reference with the settings above.
4. A resampled 1 kHz tone gives THD+N ≤ −100 dB.
5. Files named `Florida!!!.opus`, `Habits (Stay High).opus`, and `Sigur Rós.flac` decode.

The thresholds in items 3 and 4 are inferred. If the measurement shows they are wrong, revise them with the recorded evidence. Do not loosen them silently.

Run the tests. They fail because the decoder does not exist.

## Acceptance criteria

- [x] Every listed format decodes with no ffmpeg on PATH.
- [x] Opus decode matches the libopus reference within the recorded 1e-4 tolerance (revised from "exactly"; measured max |diff| 1.5e-5).
- [x] The resampler numbers are recorded in `Outcome`.
- [x] The new tests failed first for the expected reason.
- [x] `pnpm verify` passes.

## Outcome detail

Both sides of the Opus comparison link libopus 1.6.1, and both are float builds (our version
string is `libopus 1.6.1`, with no `-fixed` suffix). They are separate binaries, so they are not
bit-exact. The difference is broad floating-point rounding, not quantization: 575,765 of 576,000
samples differ, the smallest nonzero difference is 4.66e-10, there are 24,008 distinct difference
values, and only 1,193 are multiples of 2^-16. Separate builds contract multiply-adds differently
and select different SIMD paths, and libopus's recursive synthesis carries the rounding across the
301 packets. The stack already expects this for the ONNX output (§C: compare with a tolerance,
never a byte hash).

The container trims are read by `decode::matroska` (`CodecDelay`, `DiscardPadding`) and
`decode::mp4` (edit-list `media_time`), each unit-tested on hand-built bytes. Symphonia surfaces
none of them: its Matroska reader builds untrimmed packets, and its MP4 reader parses `elst` but
does not apply it.

The resampler gate scores an analytic reference. rubato's `output_delay()` truncates
`nbr_points * ratio / 2`, so `process_all` left a 0.38-sample offset. `decode::resample` now
measures the leftover fraction from an impulse and removes it with a windowed-sinc
fractional-delay filter. The offset is fixed in production code, not in the test.

For this deep worktree path only, `CARGO_TARGET_DIR` must be set: the vendored cmake build for
libopus exceeds MAX_PATH under the worktree. It is the only non-committed environment variable the
gate needs. With it set and every `CMAKE_*` variable unset, a clean build exits 0.

The trailing MP4 trim changed on 2026-10-10. The orchestrator ruled that the decoder trims the
edit list's presentation duration as well as its start. Ruling-03 item 2 had said to trim the start
only, and the review's F6 text assumed that. That rule rested on the old conda ffmpeg, which
trims only the start; the pinned BtbN build trims both ends to the source length, which is the
gapless-correct result. `decode::mp4` now returns the start and the summed non-empty edit
durations, `media_time` converted with the media timescale from `mdhd` and `segment_duration` with
the movie timescale from `mvhd`.

Fix round 1 (2026-10-10) resolved the eight blocking review findings. F1: a hostile
`DiscardPadding` returns a typed `Malformed` error, with checked arithmetic in both readers. F2:
both container readers stream through a bounded `Read + Seek` and never load the whole file. F3:
the selected container track drives the trim. F4: each block's signed `DiscardPadding` trims that
block, positive at its end and negative at its start. F5: an unknown-size Segment's clean end is
end of stream, and a truncated WebM still errors. F6: an initial empty MP4 edit is skipped, so the
following start trim applies. F7: mapping-family 1 and 255 Opus streams decode with libopus's
multistream decoder, the `OpusHead` mapping is validated, and a bad header returns a typed error.
F8: the analytic gate trims only the measured stop transient and requires the scored band to reach
19.9 kHz, and a steady 20 kHz tone gate scores 100.4 dB. Each finding has a red run saved before
its fix.

Fix round 2 (2026-10-10) resolved the five blocking findings and three gaps. B1: `UnexpectedEof` is
an error by default, accepted as a clean end only when the Matroska scanner proved the stream ends
at a valid boundary; the live-WebM acceptance is pinned to 288000 frames. B2: the scanner records
the physical file length once and bounds every header and body against the file and the nearest
finite ancestor, checks each block-header read against the block end, accepts a physical end of file
only under an unknown-size master, and refuses a block table past its budget. B3: the Opus mapping
families are scoped to 0, 1, and 255 within the backend's eight-channel limit, the parser enforces
the RFC limit N + M <= 255, and the version's upper four bits are validated; a legal header outside
the scope returns `UnsupportedFormat` naming the family, the channel count, and the stream counts.
B4: the signed Q7.8 `OpusHead` output gain is applied once as a float scale, with no clipping. B5
and C1: the MP4 edit-list reader takes the selected track ID, matches `tkhd`, parses `media_rate`,
and returns `Result<Option<EditListTrim>, DecodeError>`; initial empty edits followed by one
contiguous unit-rate media range are accepted, and a repeated, discontinuous, or non-unit-rate
sequence is a typed `UnsupportedFormat` error. C2: `AudioBuffer::channels` documents the codec's
native (Vorbis) channel order, and the six-channel test checks the fixed expected permutation. C3:
the steady 20 kHz tone test uses the measured 96-frame margin.

## Verify

```powershell
cargo test -p melomae-core decode
pnpm verify
```