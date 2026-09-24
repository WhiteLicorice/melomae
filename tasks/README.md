# Task board

This board defines the build order for Melomae. The MVP task records live in `tasks/mvp`. Future-deferred work lives in `tasks/backlog`. One numbered file is one unit of work.

`docs/melomae-srs.md` defines product behavior. `docs/melomae-stack.md` defines implementation choices and records the measured baselines. If a task conflicts with either document, stop and correct the task before implementation.

## Executor protocol

When the prompt says `Continue with next task`, do exactly this:

1. Read this file.
2. Read `AGENTS.md`, `docs/melomae-srs.md`, and `docs/melomae-stack.md`.
3. Read the root `README.md` when it exists.
4. Run `git status --short`. Before task 00 creates the repository, record "no repository".
5. Run `git log --oneline -5` when the repository exists.
6. Select the lowest `TODO` task in `tasks/mvp` whose dependencies are all `DONE`.
7. Do not execute `BLOCKED`, `CANCELLED`, or human-owned actions.
8. Open the selected task and read every cited SRS and stack section.
9. Record the base commit, working-tree state, and current gate result in `Started`.
10. Set the task and board status to `IN PROGRESS`.
11. Write the required failing test first. Run it. Confirm that it fails for the expected reason.
12. Implement only the selected task.
13. Run every command under `Verify`.
14. Run the gate. From task 00 onward, the gate is `pnpm verify`. Before that, the gate is `python tasks/validate_board.py` and `python -m unittest discover -s tasks -p "test_*.py"`.
15. Set the task and board status to `DONE` only when all gates pass.
16. Record the result and relevant gate counts in `Outcome`.
17. Stop and report. Do not start another task unless the user asks for continued execution.

For documentation-only or one-line configuration work, record the manual check instead of a test. Do not manufacture a failing test.

If the tree contains unrelated changes, preserve them. Do not stage, revert, or edit them. If a dependency is missing, leave the task `TODO` and report the mismatch.

The Apollo clone at `C:\Lab\Apollo` is read-only. Do not write to it. When you run Python from it, set `PYTHONDONTWRITEBYTECODE=1`.

### Spike tasks

Tasks 01 and 02 are spikes. Each one ends with `GO` or `NO-GO` in `Outcome`, with the measured evidence.

On `NO-GO`, do these steps:

1. Set every task that depends on the spike to `BLOCKED`.
2. Write a board amendment that names the replacement approach.
3. Stop and report to the owner. Do not start the replacement before the owner approves the amendment.

## Human-input protocol

Use this protocol when a selected task needs an unfinished item from `mvp/bootstrap-task.md`:

1. Continue all safe work that does not need the missing input.
2. Stop only when the missing input prevents further verified progress.
3. Keep the task `TODO`, or set it to `BLOCKED` if work already started.
4. Name the task number and the exact missing input.
5. Explain why the input is required now.
6. Check the provider's current official documentation and dashboard in the same turn.
7. Check the provider changelog when settings or commands can drift.
8. Give numbered steps that use the current dashboard labels and command syntax.
9. Give a recommended choice when a safe default exists.
10. Explain an alternative only when it changes cost, security, or product behavior.
11. State which values are secrets and where the owner must store them.
12. Never ask the owner to paste a secret into chat, Git, an issue, or a task file.
13. Ask for non-secret completion evidence, such as a setting name, status, or secret-store entry name.
14. State the exact task and verification step that will resume afterward.
15. Cite the official sources and the date checked.

Do not copy stale dashboard paths from an old task without verification. If the current instructions cannot be confirmed, name the failed source check. Do not guess.

If no eligible `TODO` task remains, inspect the lowest `BLOCKED` task whose code dependencies are `DONE`. Give the human-input instructions for that task. Do not mark it `IN PROGRESS` until the owner completes the required action.

After the owner reports completion, verify the resulting state without printing secret values. Mark the blocked task `DONE` only after its acceptance criteria and verification gate pass.

## Status legend

| Status | Meaning |
| --- | --- |
| `TODO` | An agent may select the task after all dependencies are `DONE`. |
| `IN PROGRESS` | An agent started the task. Inspect Git before continuing it. |
| `BLOCKED` | A person must supply an input, or a spike returned `NO-GO`. |
| `CANCELLED` | The task no longer ships. Its file records the reason. |
| `DONE` | The implementation and all verification gates passed. |

The validator rejects any other status value.

## Human-assigned work

| Task | Status | Notes |
| --- | --- | --- |
| [Bootstrap handoff](mvp/bootstrap-task.md) | TODO | The MusicBrainz contact, the GitHub repository, the app identifier, code signing, the YouTube risk acceptance, and the name check. Agents instruct the owner but do not perform these actions. |

## Phase 0 — Foundation and go/no-go spikes

| # | Task | Status | Depends on |
| --- | --- | --- | --- |
| 00 | [Bootstrap the Tauri workspace and the verify gate](mvp/00-bootstrap-the-tauri-workspace-and-the-verify-gate.md) | DONE | — |
| 01 | [Prove the ONNX export against stock Apollo](mvp/01-prove-the-onnx-export-against-stock-apollo.md) | TODO | 00 |
| 02 | [Prove YouTube Music search and download through yt-dlp](mvp/02-prove-youtube-music-search-and-download-through-yt-dlp.md) | TODO | 00 |

## Phase 1 — Restoration engine

| # | Task | Status | Depends on |
| --- | --- | --- | --- |
| 03 | [Decode every input format to 44.1 kHz float audio](mvp/03-decode-every-input-format-to-float-audio.md) | TODO | 00 |
| 04 | [Port chunked inference to ONNX Runtime](mvp/04-port-chunked-inference-to-onnx-runtime.md) | TODO | 01, 03 |
| 05 | [Size chunks from free memory](mvp/05-size-chunks-from-free-memory.md) | TODO | 04 |
| 06 | [Encode FLAC, WAV, and Opus output](mvp/06-encode-flac-wav-and-opus-output.md) | TODO | 03 |
| 07 | [Add the restore command-line entry](mvp/07-add-the-restore-command-line-entry.md) | TODO | 04, 05, 06 |
| 08 | [Add GPU execution with CPU fallback](mvp/08-add-gpu-execution-with-cpu-fallback.md) | TODO | 07 |

## Phase 2 — Metadata and acquisition

| # | Task | Status | Depends on |
| --- | --- | --- | --- |
| 30 | [Define the stream source interface](mvp/30-define-the-stream-source-interface.md) | TODO | 00, 03 |
| 09 | [Add the MusicBrainz client with a shared rate limiter](mvp/09-add-the-musicbrainz-client-with-a-shared-rate-limiter.md) | TODO | 00 |
| 10 | [Fetch cover art from the Cover Art Archive](mvp/10-fetch-cover-art-from-the-cover-art-archive.md) | TODO | 09 |
| 11 | [Manage the yt-dlp and Deno binaries](mvp/11-manage-the-yt-dlp-and-deno-binaries.md) | TODO | 02, 30 |
| 12 | [Match MusicBrainz tracks to YouTube Music candidates](mvp/12-match-musicbrainz-tracks-to-youtube-music-candidates.md) | TODO | 02, 09, 11, 30 |
| 13 | [Download the best audio stream](mvp/13-download-the-best-audio-stream.md) | TODO | 11, 12, 30 |

Task 30 was added after the board was created. Tasks 11 to 14 and 17 depend on it. Task numbers record creation order. The executor still selects the lowest eligible `TODO`.

Stream sources are a core requirement (SRS §5.9). Acquisition goes through the task 30 interface. YouTube Music is the only MVP source. The Bandcamp and SoundCloud sources are backlog records B-009 and B-010.

## Phase 3 — Library and pipeline

| # | Task | Status | Depends on |
| --- | --- | --- | --- |
| 14 | [Persist jobs and settings in SQLite](mvp/14-persist-jobs-and-settings-in-sqlite.md) | TODO | 00, 30 |
| 15 | [Write tags and art and file tracks into the library](mvp/15-write-tags-and-art-and-file-tracks-into-the-library.md) | TODO | 06, 09, 10 |
| 16 | [Scan the library and deduplicate by MBID](mvp/16-scan-the-library-and-deduplicate-by-mbid.md) | TODO | 14, 15 |
| 17 | [Run the job pipeline end to end](mvp/17-run-the-job-pipeline-end-to-end.md) | TODO | 07, 13, 14, 15, 16, 30 |

## Phase 4 — Desktop UI

| # | Task | Status | Depends on |
| --- | --- | --- | --- |
| 18 | [Generate typed Tauri commands and progress events](mvp/18-generate-typed-tauri-commands-and-progress-events.md) | TODO | 17 |
| 19 | [Build the app shell, theme, settings, and E2E harness](mvp/19-build-the-app-shell-theme-settings-and-e2e-harness.md) | TODO | 18 |
| 20 | [Browse MusicBrainz and enqueue releases](mvp/20-browse-musicbrainz-and-enqueue-releases.md) | TODO | 16, 19 |
| 21 | [Show the job queue with progress, ETA, and cancel](mvp/21-show-the-job-queue-with-progress-eta-and-cancel.md) | TODO | 19 |
| 22 | [Review uncertain matches](mvp/22-review-uncertain-matches.md) | TODO | 12, 21 |
| 23 | [Compare before and after audio](mvp/23-compare-before-and-after-audio.md) | TODO | 21 |
| 24 | [Restore local files and folders](mvp/24-restore-local-files-and-folders.md) | TODO | 07, 21 |
| 25 | [Add the first-run setup and the attribution screen](mvp/25-add-the-first-run-setup-and-the-attribution-screen.md) | TODO | 08, 11, 19 |

## Phase 5 — Packaging and release

| # | Task | Status | Depends on |
| --- | --- | --- | --- |
| 26 | [Publish third-party notices and the model attribution](mvp/26-publish-third-party-notices-and-the-model-attribution.md) | TODO | 03, 04, 06, 11 |
| 27 | [Build and verify the Windows installer](mvp/27-build-and-verify-the-windows-installer.md) | TODO | 25, 26 |
| 28 | [Build and verify the Linux packages](mvp/28-build-and-verify-the-linux-packages.md) | TODO | 25, 26 |
| 29 | [Run the release-candidate journey](mvp/29-run-the-release-candidate-journey.md) | TODO | 20, 21, 22, 23, 24, 27, 28 |

## Blocked tasks

| Task | Human input that clears it |
| --- | --- |
| — | No task is blocked. Tasks 09, 27, 28, and 29 need bootstrap items before they can finish. |

After a board edit, run `python tasks/validate_board.py`. The task file and its board row must agree on the title, the status, and the dependencies. Every dependency must name a task that exists.

## Deferred backlog

| # | Task | Status | Depends on |
| --- | --- | --- | --- |
| B-001 | [Assess the macOS target](backlog/B-001-assess-the-macos-target.md) | TODO | 04 |
| B-002 | [Run the Android feasibility spike](backlog/B-002-run-the-android-feasibility-spike.md) | TODO | 01, 04 |
| B-003 | [Add a spectrogram view to the compare player](backlog/B-003-add-a-spectrogram-view-to-the-compare-player.md) | TODO | 23 |
| B-004 | [Offer a faster reduced-precision model](backlog/B-004-offer-a-faster-reduced-precision-model.md) | TODO | 01, 04 |
| B-005 | [Stream decoding and encoding for very long inputs](backlog/B-005-stream-decoding-and-encoding-for-very-long-inputs.md) | TODO | 04, 05 |
| B-006 | [Add signed in-app updates](backlog/B-006-add-signed-in-app-updates.md) | TODO | 27, 28 |
| B-007 | [Use MusicBrainz streaming links as the first match source](backlog/B-007-use-musicbrainz-streaming-links-as-the-first-match-source.md) | TODO | 12, 30 |
| B-008 | [Trim non-music sections with SponsorBlock](backlog/B-008-trim-non-music-sections-with-sponsorblock.md) | TODO | 02, 12, 13 |
| B-009 | [Add a Bandcamp source](backlog/B-009-add-a-bandcamp-source.md) | TODO | 30, 17 |
| B-010 | [Add a SoundCloud source](backlog/B-010-add-a-soundcloud-source.md) | TODO | 30, 17 |

Each backlog record states why it does not block the MVP. Each record also states its promotion condition.
