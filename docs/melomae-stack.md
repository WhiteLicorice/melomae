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
| `crates/melomae-core` | The whole pipeline: decode, inference, encode, MusicBrainz, matching, download, tags, library, jobs. It must not depend on any `tauri*` crate. A test enforces this. |
| `crates/melomae-cli` | A headless command line over the core. Agents and tests use it as the end-to-end path. |
| `src-tauri` | The Tauri app. It holds commands, events, and packaging config only. |

- `melomae-core::source` holds the `StreamSource` trait and the source registry (task 30). Each source lives in its own module, for example `source/youtube/`.
- Long-running work runs off the main thread. Progress goes to the UI over Tauri Channels.
- The TypeScript bindings are generated from Rust types. Ticket 18 picks tauri-specta or ts-rs.

## §C — Inference

- ONNX Runtime through the `ort` crate (2.0.0-rc.13 on 2026-09-24).
- `tools/export` (a uv Python project) builds `apollo.onnx` from the official checkpoint. It vendors the upstream model files `look2hear/models/apollo.py` and `base_model.py` unmodified from `JusperLee/Apollo@e84bcac`.
- Checkpoint: `JusperLee/Apollo` `pytorch_model.bin`, 66,541,845 bytes, SHA-256 `99d9af7f1ff20e63c393035513a655392818d66b4d7fc23d658175c1f15e8d76`. The export verifies this hash.
- Model constants: `sr=44100`, `win=20` ms (882 samples), hop 441, 80 bands, `feature_dim=256`, 6 layers (`apollo.py:207-247`).
- The model is channel-independent. `apollo.py:251-253,292,296` fold channels into the batch. The engine runs one channel at a time to halve peak memory. Ticket 01 confirms this numerically.
- The chunking algorithm is a port of `C:\Lab\Apollo\inference.py:86-221`: padded chunks, a padded-start clamp at the file end, linear crossfades, and normalized overlap-add.
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

## §E — Acquisition

All acquisition goes through the `StreamSource` trait (task 30). A source searches, maps its results to the neutral `Candidate` type, fetches a stream, and reports what it fetched in `StreamInfo`: codec, container, bitrate, sample rate, and whether it is lossless. Each source chooses its own format. No source re-encodes. The matcher, the pipeline, the store, and the UI never name a specific source. A boundary test enforces this in `melomae-core` (SRS §5.9).

### MVP source: YouTube Music

- The official yt-dlp standalone binary does YouTube Music search and download. yt-dlp is Unlicense. Its PyInstaller binaries include GPLv3+ code.
- yt-dlp needs an external JavaScript runtime for full YouTube support (yt-dlp issue #15012, since 2025.11.12). Deno (MIT) is the default runtime. The official binaries already bundle the EJS component.
- The app downloads both binaries on first run. It verifies them against their published SHA-256 sums, updates them itself, and runs them as child processes. They are not bundled in the installer.
- Format selection: `bestaudio[acodec=opus]/bestaudio[acodec^=mp4a]`. YouTube serves no MP3.
- The owner rejected rustypipe (last crate release 0.11.4 on 2025-04-23) and YoutubeExplode (maintenance mode, no YouTube Music search) on 2026-09-24.

### Backlog sources

Checked on 2026-09-24. yt-dlp supports both sources (`supportedsites.md`), so the managed binary from task 11 can serve them.

- **Bandcamp (B-009).** The public stream is `mp3-128` (`yt_dlp/extractor/bandcamp.py:460`). A free-download page offers other encodings, which can be lossless (`bandcamp.py:223-280`). Purchased downloads need the user's account.
- **SoundCloud (B-010).** The formats are AAC, Opus, and MP3 over HTTP or HLS (`yt_dlp/extractor/soundcloud.py:109`). AAC reaches 256 kb/s for premium accounts (`:355`). The original WAV needs authentication (`:597`, `:630`).

### SponsorBlock

The MVP does not use SponsorBlock (SRS §5.11). Backlog record B-008 holds the option. Evidence, checked on 2026-09-24:

- The matcher prefers song results from Topic channels. They carry the release audio, and SponsorBlock segments belong to uploaded videos. This is inferred. Spike 02 measures how often a track has only a music-video candidate.
- The duration gate rejects a music video with extra sections. The track goes to review, so wrong audio never reaches the library.
- `--sponsorblock-remove` is a yt-dlp post-processing step, and the yt-dlp README says ffmpeg is "Required for … various post-processing tasks." Melomae ships without ffmpeg.
- The useful category is "Music: Non-Music Section" (`music_offtopic`). It covers "music not present in the official or Spotify music release."
- "The API and database follow CC BY-NC-SA 4.0 unless you have explicit permission." Each lookup also sends the video ID to a third party.

## §F — Metadata

- MusicBrainz WS/2 JSON API. The limit is 1 request per second per IP. The User-Agent must be `Melomae/<version> ( <contact> )`. The owner supplies the contact (bootstrap item 1).
- Cover Art Archive for front covers. Ticket 10 confirms its rate policy.
- lofty reads and writes tags: Vorbis comments for FLAC and Opus, ID3 for WAV.
- Tag names follow the MusicBrainz Picard mapping. Ticket 15 confirms it against the live Picard documentation.

## §G — Storage

- SQLite through `rusqlite` with the bundled SQLite. The database lives in the app data directory.
- `PRAGMA user_version` numbers the schema migrations.
- The database holds settings, jobs, and the library index.

## §H — Playback

- Before/after playback is native Rust (cpal or rodio). It does not use the webview.
- Reason: Linux WebKitGTK plays audio through GStreamer, and its codec support depends on installed plugins. Native playback behaves the same on both platforms.

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
