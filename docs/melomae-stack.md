# Melomae — stack and evidence

This document defines how Melomae is built. `docs/melomae-srs.md` defines what it does. If a task conflicts with this document, stop and correct the task before you implement it.

**Version rule.** Every version in this document was current on 2026-09-24. Confirm the current version from its primary registry before you install it. Record the version you install in the task `Outcome`.

**Apollo source.** The owner's clone is `C:\Lab\Apollo`. It is read-only. Do not write to it. When you run Python from it, set `PYTHONDONTWRITEBYTECODE=1`.

## §A — Shell

- Tauri 2 desktop app. The UI runs in the system webview: WebView2 on Windows, WebKitGTK 4.1 on Linux.
- React, Vite, and TypeScript in strict mode.
- TanStack Query for server-state caching. TanStack Virtual for long lists.
- pnpm workspace. The `packageManager` field pins pnpm. `@tauri-apps/cli` is a dev dependency, not a global tool.
- The owner rejected Electron, Avalonia, Flutter, and Slint/iced on 2026-09-24. Tauri with React gives the largest ecosystem and the best agent coverage. The heavy work runs in Rust.
- Known risk: WebKitGTK on Linux is slower than WebView2 and has codec gaps. Audio playback does not use the webview (§H).

## §B — Rust workspace

| Crate | Role |
|---|---|
| `crates/melomae-core` | The whole pipeline: decode, inference, encode, triage, tags, provenance, jobs, and history. It must not depend on any `tauri*` crate. A test enforces this. |
| `crates/melomae-cli` | A headless command line over the core. Agents and tests use it as the end-to-end path. |
| `src-tauri` | The Tauri app. It holds commands, events, and packaging config only. |

- Long-running work runs off the main thread. Progress goes to the UI over Tauri Channels.
- The TypeScript bindings are generated from Rust types. Ticket 18 picks tauri-specta or ts-rs.

## §C — Inference

- ONNX Runtime through the `ort` crate (2.0.0-rc.13 on 2026-09-24).
- `tools/export` (a uv Python project) builds `apollo.onnx` from the official checkpoint. It vendors the upstream model files `look2hear/models/apollo.py` and `base_model.py` unmodified from `JusperLee/Apollo@e84bcac`.
- Checkpoint: `JusperLee/Apollo` `pytorch_model.bin`, 66,541,845 bytes, SHA-256 `99d9af7f1ff20e63c393035513a655392818d66b4d7fc23d658175c1f15e8d76`. The export verifies this hash.
- Model constants: `sr=44100`, `win=20` ms (882 samples), hop 441, 80 bands, `feature_dim=256`, 6 layers (`apollo.py:207-247`).
- The model is channel-independent. `apollo.py:251-253,292,296` fold channels into the batch. The engine runs one channel at a time to halve peak memory. Ticket 01 confirms this numerically (`docs/evidence/01-onnx-spike.md`).
- Known limitation, accepted by the owner on 2026-09-25: the output is bit-exact on one machine with one thread count, but not across machines. ONNX Runtime CPU output moves by up to 3.4e-5 with the intra-op thread count, which is set to the physical core count. It can also move with the CPU instruction set (inferred). GPU output differs from CPU by up to 7.2e-4 (§K). Tests compare with a tolerance, never a byte hash.
- ONNX Runtime peak memory is about 0.36 GB plus 0.58 GB per channel-second (ticket 01). This is about 2.2 times the torch figure in §K.
- The chunking algorithm is a port of `C:\Lab\Apollo\inference.py:86-221`: padded chunks, a padded-start clamp at the file end, linear crossfades, and normalized overlap-add.
- The pipeline calls restore through a file-level `RestoreBackend` trait (task 04). It takes one file's 44.1 kHz float channels, the settings, a progress callback, and a cancel flag. It returns the restored channels. The local ONNX Runtime engine is the only implementation. Chunking stays inside it. The MVP has no network backend (SRS §2.4). Backlog B-012 holds the hosted option.
- GPU: ticket 08 picks the execution provider for each OS. Candidates: DirectML (in maintenance mode, WinML is its successor), CUDA, and WinML.
- Fallback: if spike 01 returns NO-GO, a Python sidecar (standalone Python plus a CPU torch wheel) replaces ONNX Runtime. A board amendment must define it first.

## §D — Audio I/O

| Job | Library | License |
|---|---|---|
| Demux MP4, Ogg, MKV/WebM, WAV, AIFF | symphonia | MPL-2.0 |
| Decode MP3, AAC, ALAC, FLAC, Vorbis, PCM | symphonia | MPL-2.0 |
| Decode Opus | libopus (reference decoder), built from vendored source | BSD-3-Clause |
| Resample | rubato, high-quality sinc | MIT |
| Encode FLAC 24-bit | a pure-Rust encoder, or libFLAC | ticket 06 confirms |
| Encode Opus | libopus, in an Ogg container | BSD-3-Clause |

- Symphonia has no Opus decoder. Its Opus work was an open WIP pull request (#398) on 2026-09-24. Do not use a partial Opus decoder.
- Opus always decodes at 48 kHz. Melomae resamples it to 44.1 kHz for Apollo. The owner's old pipeline resampled with ffmpeg swr at `filter_size=256:phase_shift=24:cutoff=0.98` (`C:\Lab\Apollo\inference2.py:39`). Ticket 03 measures rubato against that reference.
- The shipped app does not use ffmpeg. The conda ffmpeg is a dev-only test oracle.

## §E — Acquisition (removed)

Melomae does not acquire audio (SRS §1.5). On 2026-10-10 the owner removed YouTube Music, yt-dlp, Deno, the `StreamSource` interface, and the Bandcamp, SoundCloud, and SponsorBlock records. Tasks 02, 11, 12, 13, and 30 and records B-007 to B-010 are `CANCELLED`. Commit `6f91149` holds the removed evidence. `docs/evidence/2026-10-10-pivot.md` records the reasons.

## §F — Metadata

- lofty reads and writes tags: ID3 for MP3 and WAV, Vorbis comments for FLAC, Opus, and Ogg, and MP4 atoms for M4A.
- Task 15 confirms at execution time how lofty exposes custom fields (ID3 `TXXX`, any Vorbis key, MP4 freeform atoms), multi-value fields, and picture types. If lofty can't carry one of them through its generic tag layer, task 15 uses the format-specific API for it. Record the result in task 15.
- The field mapping between formats follows the MusicBrainz Picard tag mapping. Task 15 confirms it against the live Picard documentation. Reading that documentation is not use of the MusicBrainz web service.
- Melomae calls no online metadata service. The MusicBrainz web service, the Cover Art Archive, and AcoustID are free for non-commercial use only (`docs/evidence/2026-10-10-pivot.md`).

## §G — Storage

- SQLite through `rusqlite` with the bundled SQLite. The database lives in the app data directory.
- `PRAGMA user_version` numbers the schema migrations.
- The database holds settings, jobs, triage results, and history.

## §H — Playback

- Before/after playback is native Rust (cpal or rodio). It does not use the webview.
- Reason: Linux WebKitGTK plays audio through GStreamer, and its codec support depends on installed plugins. Native playback behaves the same on both platforms.
- Volume matching uses EBU R128 integrated loudness (SRS §10.4). Task 23 picks the crate. The `ebur128` crate is the candidate. Confirm it at execution time. The conda ffmpeg `ebur128` filter is a dev-only oracle.

## §I — Gate

`pnpm verify` is the gate from ticket 00 onward. It runs these checks in this order:

1. `python tasks/validate_board.py`
2. `python -m unittest discover -s tasks -p "test_*.py"`
3. The frontend typecheck, lint, Vitest, and build
4. `cargo fmt --check`
5. `cargo clippy --workspace -- -D warnings`
6. `cargo test --workspace`
7. `uv run --project tools pytest`

Ticket 18 adds `pnpm bindings:check`. Ticket 26 adds `cargo deny check licenses`. Before ticket 00 is `DONE`, the gate is steps 1 and 2.

## §J — Packaging

- Windows: a per-user NSIS installer. It needs no admin rights. It uses the WebView2 download bootstrapper. Windows 11 already ships WebView2 (version 153.0 on the dev machine).
- Linux: an AppImage and a .deb, built on Ubuntu 22.04 against WebKitGTK 4.1.
- The installer bundles the ONNX Runtime libraries and `apollo.onnx` as resources.
- The release build makes `apollo.onnx` with `tools/export` from the pinned checkpoint.
- Size budget: 150 MB for the Windows installer.

## §K — Evidence and baselines

Measured on 2026-09-24 on the owner's machine. All runs used `C:\Lab\Apollo\asserts\input_wav.wav`, which is stereo, 44.1 kHz, and 6 s long, with SHA-256 `3c9a053913c3016b493ddc0b92e21a4e682e8fb5f872dafc945154155bddf771`. The runs computed in memory and wrote no files.

| Item | Value |
|---|---|
| CPU | AMD Ryzen 7 6800H, 8 cores, 16 threads |
| RAM | 15.2 GB |
| GPU | NVIDIA GeForce RTX 3060 Laptop, 6 GB, driver 610.74 |
| Python env | `look2hear_win`: Python 3.10.21, torch and torchaudio 2.11.0+cu130, numpy 1.26.4 |
| torch CPU, 8 threads, 1 s stereo | 3.15 s, RTF 3.15 |
| torch CPU, 8 threads, 3 s stereo | 8.09 s, RTF 2.70 |
| CPU peak working set | 0.63 GiB after model load, 1.69 GiB at 2 s, 4.03 GiB at 6 s |
| CPU memory slope | about 0.585 GiB per stereo second (about 0.29 GiB per channel-second) |
| torch CUDA, 1 s | RTF 0.244, peak VRAM 0.46 GiB |
| torch CUDA, 6 s | RTF 0.155, peak VRAM 2.45 GiB |
| CUDA vs CPU output, 6 s | max abs 7.201e-4, RMSE 6.380e-5 |

Derived estimates. These are inferred, not measured:

- The chunk compute overhead is (c + 2p) / (c − o) for chunk c, pad p, and overlap o. With c = 20 s, p = 1 s, and o = 1 s, the overhead is about 1.16.
- A 4-minute song on this CPU takes about 2.70 × 1.16 × 4 min ≈ 12.5 min. A 12-track album takes about 2.5 hours.
- The model costs about 250 GFLOP per channel-second. This figure comes from the layer shapes in `apollo.py:73-80,149-153,174-177`.

Facts checked in the Apollo clone:

- The briefer's "known facts" are out of date. `inference.py` is already device-agnostic (`:19-32`). It already uses soundfile for I/O (`:62-83`). It already has padded, chunked inference (`:140-221`).
- The local `LICENSE` is CC BY-SA 4.0. The Hugging Face card (`Apollo/README.md`) says `license: cc-by-sa-4.0`.
- The model math matches upstream `e84bcac`. The only local change (`32a2d35`) removes a `print`.
- The Apple M3 CPU figure in `MACOS_ARM64.md:82` (316 s for 6 s) does not reproduce on this x86 machine.
- A pre-existing flaw: the owner's old script `inference2.py:87-133` joins 10 s segments with no crossfade and no edge padding. Each chunk keeps its degraded start edge, and the last segment is padded with silence. Melomae ports upstream's padded algorithm instead.

The dev machine's toolchain on 2026-09-24: rustc and cargo 1.95.0 (MSVC), Visual Studio 2022 Community with VC tools, Node 24.9.0 through nvm-windows, npm 11.6.0, uv 0.12.11, Python 3.14.7, conda 26.7.1, gh 2.97.0, WSL2 Ubuntu, and Docker 29.8.0.

- `pnpm` on PATH is broken. It resolves to the corepack 0.34.0 shim at `C:\nvm4w\nodejs\pnpm`. That shim looks for `bin\pnpm.cjs`, but pnpm 12.6.0 ships `bin\pnpm.mjs`. Ticket 00 repairs this.
- The Tauri CLI is not installed globally.
