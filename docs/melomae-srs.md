# Melomae — product requirements

This document defines product behavior. `docs/melomae-stack.md` defines implementation choices. If a task conflicts with this document, stop and correct the task before you implement it.

Melomae is a free, open-source desktop app. It builds an offline music library. The user picks music on MusicBrainz. Melomae finds the audio on YouTube Music, downloads it, restores it with Apollo, tags it, and files it into the library. Melomae can also restore audio files that the user already owns.

## §1 — Purpose and the Apollo caveat

1. Melomae turns a MusicBrainz selection into finished, tagged library files.
2. Restoration is a first-class pipeline stage. It is not an add-on.
3. Apollo generates plausible mid- and high-frequency content from patterns in its training data. It does not recover the data that lossy compression discarded. The result is a reconstructed approximation, not a bit-perfect restoration.
4. The app states the caveat in item 3 in the first-run flow and on the About screen. The copy must not claim "lossless", "original quality", or "recovered".

## §2 — Platforms

1. The MVP targets Windows 10 and 11 (x86_64) and Linux (x86_64, glibc 2.35 or later).
2. macOS is a stretch target. It stays in the backlog until a feasibility record promotes it.
3. Android is exploratory. A time-boxed spike must return GO before any Android app work starts.
4. The app is a desktop app. There is no server mode. The backend runs inside the app.

## §3 — Install and first run

1. A non-technical user installs Melomae without Python, conda, ffmpeg, or a command line.
2. The installer bundles the Apollo model. The first restore works offline.
3. The first-run flow does these things, in this order:
   1. Show the Apollo caveat (§1.3) and the user-responsibility notice (§11.5).
   2. Ask for the library folder.
   3. Download the YouTube tools (yt-dlp and a JavaScript runtime) with visible progress.
   4. Measure the machine's restore speed and show the expected minutes for a 4-minute track.
4. If the tool download fails, local file restore (§9) still works. The app tells the user which features need the tools.

## §4 — Browse

1. The user searches MusicBrainz for artists, albums, and tracks.
2. An artist page lists the discography, grouped by release-group type.
3. The user picks a specific release (edition) of a release group.
4. A tracklist shows position, title, artist credit, and duration before the user commits.
5. Tracks and releases that exist in the library show an "In library" mark (§8.6).
6. The user can queue a whole release or single tracks.

## §5 — Match and download

1. For each queued track, Melomae searches YouTube Music.
2. A candidate passes only when its duration is within max(3 s, 2%) of the MusicBrainz duration.
3. Candidates marked live, remix, edit, radio edit, instrumental, karaoke, cover, or sped-up lose score. This rule does not apply when the MusicBrainz title or disambiguation contains the same word.
4. Each track gets one of three results: `matched`, `uncertain`, or `none`. `uncertain` and `none` tracks go to the review queue.
5. In the review queue, the user can accept a candidate, choose another candidate, skip the track, open the candidate in a browser, or paste a URL.
6. Each source downloads its best available stream without re-encoding. For YouTube Music, Opus comes first and AAC is the only fallback. The job records the source, format, codec, and bitrate. The queue shows them.
7. The download never re-encodes the stream.
8. The user can cancel a download. A cancel leaves no child process and no partial file in the library.
9. Melomae gets audio through a stream-source interface. YouTube Music is the only MVP source. A new source, such as Bandcamp or SoundCloud, must not change the matcher, the pipeline, the store, or the UI beyond registering the source.
10. A pasted URL goes to the source that supports it. An unsupported URL shows an error.
11. The MVP does not cut segments out of downloads. The duration gate (item 2) sends candidates with non-music sections to review. Backlog record B-008 holds the SponsorBlock option.

## §6 — Restore

1. The restored output matches stock Apollo within the tolerances in §12.1.
2. CPU-only restore is a first-class path. It must finish without error on a machine with no GPU.
3. GPU restore is a bonus. When the GPU path fails, the job retries on CPU and tells the user.
4. The app sizes restore chunks from the free memory at job start. A long file must not exhaust RAM.
5. The job shows progress (chunk n of m) and an ETA.
6. The user can cancel a restore. The cancel takes effect within one chunk. It leaves no partial output.
7. The user can turn restore off for a job or by default. Then the job keeps the downloaded audio and encodes it to the chosen format.
8. Melomae resamples every input to 44.1 kHz before inference. Apollo expects 44.1 kHz.
9. The device setting has three values: Auto, CPU, and GPU.
10. When a source delivers a lossless stream, the job skips restore by default. Restore would only add generated content to audio that lost nothing. The user can force a restore. YouTube Music never delivers lossless audio, so this rule matters only for later sources.

## §7 — Output formats

1. FLAC, 24-bit, 44.1 kHz. This is the default.
2. WAV, 32-bit float, 44.1 kHz.
3. Opus in Ogg, 48 kHz, 256 kb/s VBR.
4. For an integer format, samples outside ±1.0 are clamped. The job reports how many samples were clamped.
5. Melomae writes each output to a temporary file first. It moves the file into place only when the file is complete.

## §8 — Tag and organize

1. Melomae writes these tags: title, artist, album artist, album, track number and total, disc number and total, date, original date, genre, ISRC, and the MusicBrainz recording, track, release, release-group, artist, and album-artist IDs.
2. Tag names follow the MusicBrainz Picard mapping for each format.
3. Melomae embeds the front cover from the Cover Art Archive. A missing cover is not an error.
4. The path is `Album Artist/Album/NN Title.ext`. A multi-disc release uses `D-NN Title.ext`.
5. Names are safe on Windows and Linux. Reserved characters and reserved names are replaced. A name collision gets a ` (2)` suffix.
6. Melomae scans the library and indexes MusicBrainz IDs:
   - The same recording on the same release is "in library". Melomae skips it at queue time.
   - The same recording on a different release is allowed. The app flags it.

## §9 — Local file restore

1. The user drops or picks files or folders.
2. Folders are searched recursively for mp3, m4a, wav, aiff, aif, flac, opus, ogg, and webm files.
3. The output mirrors the input folder tree inside the chosen output folder.
4. An output that already exists is skipped.
5. A failed file does not stop the batch.
6. Tags from the source file are copied to the output.

## §10 — Before and after compare

1. For a finished job, the user plays the source and the restored audio.
2. The user switches between the two at the same playback position. The switch takes 50 ms or less.
3. The user can seek, play, and pause.
4. The app keeps job sources in a cache so the compare works. The user can turn this off and can clear the cache.

## §11 — Licensing, attribution, and risk

1. Melomae code is licensed GPL-3.0-or-later.
2. The Apollo code and weights are licensed CC BY-SA 4.0. The local `LICENSE` in the Apollo clone and the Hugging Face model card both confirm this. There is no NonCommercial term.
3. The app and its source credit the Apollo creators, name CC BY-SA 4.0, link the license and the source, and state the modifications (conversion to ONNX).
4. Melomae must not suggest that the Apollo authors, Tsinghua University, or Tencent AI Lab endorse it (CC BY-SA 4.0 §2(a)(6)).
5. Downloading from YouTube can break YouTube's Terms of Service and copyright law. The first-run flow states that the user is responsible for what they download. The owner accepts the release risk before a public release.
6. The app ships a third-party notice file for every bundled component.

## §12 — Quality targets

1. **Parity.** Compared with stock Apollo on the same input and the same chunk settings: max absolute difference ≤ 1e-3 and RMSE ≤ 1e-4. Reference: stock torch CUDA vs CPU differ by max abs 7.2e-4 and RMSE 6.4e-5 on the 6 s fixture.
2. **CPU speed.** ONNX Runtime CPU runs at no more than 1.2 × the torch CPU real-time factor (RTF) on the same machine. Baseline: torch CPU RTF 2.70 on a Ryzen 7 6800H.
3. **Memory.** Peak restore memory stays within 50% of the RAM available at job start.
4. **Installer size.** The Windows installer is 150 MB or smaller.
5. **Compare switch.** 50 ms or less (§10.2).
6. Each ticket records measured values against these targets in its `Outcome`.
