# Melomae — product requirements

This document defines product behavior. `docs/melomae-stack.md` defines implementation choices. If a task conflicts with this document, stop and correct the task before you implement it.

Melomae is a free, open-source desktop app. It restores a music collection that the user already owns. The user adds folders. Melomae shows which files can gain from restoration. It restores the selected files with Apollo on the user's own machine. It keeps every tag and picture, and it writes the output into a folder tree that mirrors the input. No audio leaves the machine.

`docs/evidence/2026-10-10-pivot.md` records why the product changed on 2026-10-10.

## §1 — Purpose and the Apollo caveat

1. Melomae turns folders of lossy music files into restored files with the same tags, pictures, and folder structure.
2. Restoration is a first-class pipeline stage. It is not an add-on.
3. Apollo generates plausible mid- and high-frequency content from patterns in its training data. It does not recover the data that lossy compression discarded. The result is a reconstructed approximation, not a bit-perfect restoration.
4. The app states the caveat in item 3 in the first-run flow and on the About screen. The copy must not claim "lossless", "original quality", or "recovered".
5. Melomae does not download audio. The README and the app do not name, link to, or suggest any download tool or streaming service.

## §2 — Platforms

1. The MVP targets Windows 10 and 11 (x86_64) and Linux (x86_64, glibc 2.35 or later).
2. macOS is a stretch target. It stays in the backlog until a feasibility record promotes it.
3. Android is exploratory. A time-boxed spike must return GO before any Android app work starts.
4. The app is a desktop app. There is no server mode. Restore runs on the user's CPU or GPU.

## §3 — Install and first run

1. A non-technical user installs Melomae without Python, conda, ffmpeg, or a command line.
2. The installer bundles the Apollo model. The first restore works offline.
3. The first-run flow does these things, in this order:
   1. Show the Apollo caveat (§1.3).
   2. Ask for the default output folder.
   3. Measure the machine's restore speed. Show the expected minutes for a 4-minute track on CPU, and on GPU when one exists.
4. The app needs no network access, at first run or later.

## §4 — Triage

1. The user drops or picks files or folders. Folders are searched recursively for mp3, m4a, wav, aiff, aif, flac, opus, ogg, and webm files.
2. For each file, Melomae records the container, codec, bitrate, sample rate, bit depth when known, duration, and the measured spectral cutoff.
3. Each file gets one verdict:
   - `restore`: a lossy source whose cutoff is below the high-bitrate threshold.
   - `low-benefit`: a lossy source whose cutoff is at or above the high-bitrate threshold. A 320 kb/s MP3 usually gets this verdict.
   - `suspect-lossless`: a lossless container whose cutoff looks like a lossy source.
   - `lossless`: a lossless container with full bandwidth.
   - `unreadable`: the file does not decode. The verdict shows the reason.
4. Measured fixtures set the thresholds. Task 16 records them. The verdicts are estimates, and the app says so.
5. The triage screen selects `restore`, `suspect-lossless`, and `low-benefit` files by default. The verdict is an estimate, so the user decides whether a `low-benefit` file gains from restore. Restore would only add generated content to audio that lost nothing, so `lossless` files start cleared. The user can select or clear any file.
6. Triage runs offline. A rescan reads only the files whose size or modification time changed.
7. Triage never changes an input file.

## §5 — Metadata preservation

1. Melomae copies every tag from the source to the output. This includes standard fields, sort fields, multi-value fields, MusicBrainz IDs, and custom fields (ID3 `TXXX`, Vorbis comments with any key, and MP4 freeform atoms).
2. When the output format differs from the source format, Melomae maps each field to its equivalent in the target format. The mapping follows the MusicBrainz Picard tag mapping. Task 15 confirms it against the live Picard documentation.
3. A custom field with no standard equivalent stays a custom field with the same name in the target format.
4. Melomae copies every embedded picture with its picture type and description. A front cover stays a front cover.
5. Melomae never drops a field silently. When the target format can't hold a field, the job report lists it.
6. Melomae uses no online metadata service. It does not add, correct, or look up tags.
7. FLAC and Opus outputs use Vorbis comments. WAV output uses ID3.

## §6 — Restore

1. The restored output matches stock Apollo within the tolerances in §12.1.
2. CPU-only restore is a first-class path. It must finish without error on a machine with no GPU.
3. GPU restore is a bonus. When the GPU path fails, the job retries on CPU and tells the user.
4. The app sizes restore chunks from the free memory at job start. A long file must not exhaust RAM.
5. The job shows progress (chunk n of m) and an ETA.
6. The user can cancel a restore. The cancel takes effect within one chunk. It leaves no partial output.
7. The user can turn restore off for a job or by default. Then the job encodes the source audio to the chosen format and keeps its tags.
8. Melomae resamples every input to 44.1 kHz before inference. Apollo expects 44.1 kHz.
9. The device setting has three values: Auto, CPU, and GPU.
10. Melomae restores every file that the user selects, whatever its verdict. The verdict only sets the default selection (§4.5).

## §7 — Output formats

1. FLAC, 24-bit, 44.1 kHz. This is the default.
2. WAV, 32-bit float, 44.1 kHz.
3. Opus in Ogg, 48 kHz, 256 kb/s VBR.
4. For an integer format, samples outside ±1.0 are clamped. The job reports how many samples were clamped.
5. Melomae writes each output to a temporary file first. It moves the file into place only when the file is complete.

## §8 — Output layout and provenance

1. The output mirrors the input folder tree inside the chosen output folder.
2. The output keeps the input file name. Only the extension changes to match the output format.
3. When an output already exists, Melomae skips the file. The job report says so.
4. Melomae never writes to, moves, or deletes an input file.
5. Melomae writes a provenance field into every output. The field is ID3 `TXXX:MELOMAE_PROVENANCE` for WAV and the Vorbis comment `MELOMAE_PROVENANCE` for FLAC and Opus.
6. The provenance value is one line of space-separated `key=value` pairs: `melomae` (the app version), `model` (`apollo`, or `none` when restore is off), `sha256` (the model checkpoint hash), `device`, `chunk` (chunk, overlap, and pad seconds, as `20/1/1`), and `date` (UTC, ISO 8601).
7. Provenance never replaces or edits another field. When the source already has `MELOMAE_PROVENANCE` values from earlier restores, Melomae keeps them and adds the new value.
8. Melomae keeps a history in its database. Each row holds the input path, the input SHA-256, the output path, the verdict, the settings, the provenance value, and the result.

## §9 — Batch jobs

1. A batch is the set of files that the user selects on the triage screen.
2. A failed file does not stop the batch.
3. The user can cancel one job or the whole batch.
4. The queue shows each job's stage, progress, ETA, and verdict.
5. At the end, a batch report lists the restored, skipped, and failed files, and the fields from §5.5.

## §10 — Before and after compare

1. For a finished job, the user plays the source and the restored audio.
2. The user switches between the two at the same playback position. The switch takes 50 ms or less.
3. The user can seek, play, and pause.
4. Volume matching: Melomae measures the integrated loudness (EBU R128) of both versions. It plays the louder version with a negative gain offset, so both play at the same loudness. The offset applies to playback only. It never changes a file. The player shows the offset in dB.
5. Volume matching is on by default. The user can turn it off outside blind mode.
6. Blind mode (ABX): the player hides which version is which. For each trial, X is A or B, chosen at random. The user picks the version that X matches. The user runs as many trials as they want. At the end, the player shows the correct count and the probability of that count or better by guessing (a one-sided binomial test with p = 0.5).
7. Blind mode always uses volume matching.
8. The compare reads the input file from its original path. When the input file is missing or its SHA-256 differs from the history (§8.8), the compare is unavailable and says why.

## §11 — Licensing, attribution, and risk

1. **Melomae code is licensed GPL-3.0-only.** The owner decided this on 2026-10-11. It replaces GPL-3.0-or-later.
   - The app contains code translated from CC BY-SA 4.0 sources (§11.11). CC BY-SA 4.0 §3(b)(1) and §1(c) accept GPLv3 as the license for that code. They accept a later GPL version only if Creative Commons lists it.
   - One license for all Melomae code keeps the project simple.
   - Git history shows the owner as the only human author. The owner can relicense alone today.
   - Accepted cost: the code has no automatic path to a later GPL version. A later relicense needs the consent of every copyright holder at that time.
   - Rejected: GPL-3.0-or-later with a §14 proxy, because it adds a decision-maker. Rejected: a mix of GPL-3.0-only and GPL-3.0-or-later files, because it is harder to keep correct.
2. The Apollo code and weights are licensed CC BY-SA 4.0. The local `LICENSE` in the Apollo clone and the Hugging Face model card both confirm this. There is no NonCommercial term.
3. The app and its source credit the Apollo creators, name CC BY-SA 4.0, link the license and the source, and state the modifications (conversion to ONNX).
4. Melomae must not suggest that the Apollo authors, Tsinghua University, or Tencent AI Lab endorse it (CC BY-SA 4.0 §2(a)(6)).
5. Melomae uses no online data service. The free terms of MusicBrainz, the Cover Art Archive, and AcoustID cover non-commercial use only. `docs/evidence/2026-10-10-pivot.md` quotes them.
6. The app ships a third-party notice file for every bundled component.
7. **The model is a separate work.** `apollo.onnx` ships beside the app under CC BY-SA 4.0. The app is GPL-3.0-only (§11.1).
   - The app loads the model at runtime from a path that the caller gives (`crates/melomae-core/src/inference/onnx.rs:3`). The installer adds the model as a resource (Stack §J). The model is not compiled into the app.
   - GPLv3 §5 calls separate works on one medium an "aggregate" (`LICENSE:243-251`). The GPL does not apply to the model.
   - The model keeps its own license. It needs no BY-SA Compatible License.
   - CC BY-SA 4.0 §2(a)(4) can make the ONNX conversion a technical modification and not Adapted Material. The decision holds under both readings.
   - The installer and the app terms must not restrict the copying of `apollo.onnx`. They must not encrypt the model (CC BY-SA 4.0 §2(a)(5)(c) and §3(b)(3)).
   - Sources: CC BY-SA 4.0 legal code. GPLv3 (`LICENSE`).
8. **The LLVM exception.** `deny.toml` admits `Apache-2.0 WITH LLVM-exception` through a per-crate exception for `target-lexicon` only. The global `allow` list does not contain it.
   - `target-lexicon 0.12.16` is the only crate under this license. It is a build dependency only: `cfg-expr` → `system-deps`, under `[build-dependencies]` of the Linux GTK `-sys` crates (`cargo tree -i target-lexicon -e normal,build --target all`). It does not ship in the installer.
   - The FSF lists Apache-2.0 as compatible with GPLv3. The LLVM exception only adds permissions.
   - A new crate under this license fails the gate until the owner reviews it.
   - Rejected: a global allow, which admits future crates without review. Rejected: `[licenses] include-build = false`, which removes every build dependency from the check. Rejected: `[graph] exclude`, which also hides the crate from the advisory, ban, and source checks.
9. **The training-audio rights.** The release ships the model and does not wait.
   - Apollo was trained on MUSDB18-HQ and MoisesDB (https://github.com/JusperLee/Apollo). MUSDB18 tracks "can only be used for academic purposes" (https://github.com/sigsep/website/blob/master/content/datasets/musdb.md). MoisesDB is CC BY-NC-SA 4.0 (https://github.com/moises-ai/moises-db/blob/main/LICENSE).
   - The owner relies on the CC BY-SA 4.0 grant for the weights. Melomae follows CC BY-SA 4.0 and does not treat the dataset terms as binding.
   - CC BY-SA 4.0 §1(i) limits the grant to rights that the Licensor has authority to license. §5(a) gives no warranty of non-infringement.
10. **The hosted backend.** The backend (B-012) can run the model on the terms in §11.9.
    - CC BY-SA 4.0 §3 applies only when You Share the material. §1(k) defines Share as providing material to the public. A model that runs on the server only is not Shared.
    - Whether restored audio is Adapted Material (§1(a)) is not settled. The backend credits Apollo and obeys §11.4.
    - If the backend sends the model to clients, §3(a) and §3(b) apply. The backend then must not encrypt the model.
    - CC BY-SA 4.0 §2(b)(2) grants no patent or trademark rights.
11. **Ported Apollo code.** The owner decided this on 2026-10-11.
    - `crates/melomae-core/src/inference/mod.rs` translates `resolve_chunking`, `chunk_starts`, `crossfade_weights`, and `run_model` from upstream `inference.py` into Rust. Upstream contributors other than the Apollo authors wrote these functions (Part 1 table).
    - `tools/export/real_dsp.py` computes the same mathematics as `Apollo.spec_band_split`, `Apollo.feature_extractor`, and `Apollo.forward` (`tools/export/vendor/look2hear/models/apollo.py:250-299`), in the same order. It copies no upstream text. The owner treats it as adapted, so that no argument is needed.
    - The upstream repository is CC BY-SA 4.0. CC BY-SA 4.0 §1(a) names translation as a form of adaptation.
    - Both files use GPL-3.0-only as the Adapter's License (§3(b)(1)). Both files carry the §3(a) attribution.
    - Before 2026-10-11, both files carried GPL-3.0-or-later and had no attribution. `mod.rs` named only a local path. Under §6(a), a breach can end the rights. Under §6(b)(1), the rights come back if the breach is cured within 30 days of its discovery. The owner discovered it on 2026-10-11. **Cure deadline: 2026-11-10.** This applies only if the files are Adapted Material.
    - Optional: the owner can ask the upstream copyright holders for other terms (§6(c)). For `inference.py`, those holders include Djshinshin and zbitouzakaria, as well as JusperLee.

## §12 — Quality targets

1. **Parity.** Compared with stock Apollo on the same input and the same chunk settings: max absolute difference ≤ 1e-3 and RMSE ≤ 1e-4. Reference: stock torch CUDA vs CPU differ by max abs 7.2e-4 and RMSE 6.4e-5 on the 6 s fixture.
2. **CPU speed.** ONNX Runtime CPU runs at no more than 1.2 × the torch CPU real-time factor (RTF) on the same machine. Baseline: torch CPU RTF 2.70 on a Ryzen 7 6800H.
3. **Memory.** Peak restore memory stays within 50% of the RAM available at job start.
4. **Installer size.** The Windows installer is 150 MB or smaller.
5. **Compare switch.** 50 ms or less (§10.2), in normal and blind mode.
6. **Tag preservation.** For the task 15 fixture, every source field and picture reads back from each output format, or the job report lists it (§5.5).
7. Each ticket records measured values against these targets in its `Outcome`.
