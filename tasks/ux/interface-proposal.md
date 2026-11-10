# Interface proposal

**Kind:** Proposal. This file is not a task record and has no board row.
**Source:** The impeccable audit of 2026-11-11 at `master` `0220b19`. The audit read the source and the task records. It did not run the app.
**Owner decision:** Not yet accepted.

This document proposes the Melomae interface as a whole: the character, the frame, each screen, the states, the keyboard contract, and the copy. `docs/melomae-srs.md` still defines product behavior. Where this proposal adds to the SRS, section 11 lists the addition as an open decision.

The current UI is the Vite and Tauri starter with a title and one sentence (`src/App.tsx:5-8`). The proposal replaces it.

Every color ratio in this file was computed from sRGB values. Nobody measured a rendered pixel. The cited SRS and stack lines were checked against the documents on 2026-11-11.

## How to use this file

1. The owner reads sections 1, 2, and 11, and records the decision in the line above.
2. Task 39 turns the accepted sections 1 and 2 into `PRODUCT.md` and `DESIGN.md`.
3. Tasks 18 to 25, 32, 40, and 41 cite the section that covers their screen.

## 1. Product character

Melomae is a quiet repair bench for a music library the user already owns. It is not a streaming app and not a media browser. The interface should feel measured, honest, and local. The user sees what the tool measured, what it will generate, and what it will leave alone.

The signature element is the **cutoff meter**. It is a thin frequency scale from 0 to 22.05 kHz. A marker sits at the file's measured spectral cutoff. A muted band above the marker shows the range that lossy compression removed. The meter appears in each triage row and again in the compare player. It makes the product's core measurement visible without a full spectrogram. The cutoff is a recorded field in `docs/melomae-srs.md:37`, and the verdict depends on it in `docs/melomae-srs.md:39-43`.

The second signature is **plain verdict language**. The internal verdict names are jargon for a non-technical user (`docs/melomae-srs.md:26`). The visible labels are:

| Internal verdict | Visible label | Shape | One-line explanation |
|---|---|---|---|
| `restore` | Can restore | Filled dot | The file lost high frequencies. Melomae can add plausible detail. |
| `low-benefit` | Little to gain | Half dot | The file kept most of its high frequencies. Restoring adds generated detail that may not help. |
| `suspect-lossless` | Looks converted | Hollow diamond | The file is marked as a full-quality format, but its high frequencies look like a converted file. Check the source. |
| `lossless` | Already full bandwidth | Filled square | The file kept its full frequency range. Restoring would add generated detail to audio that lost nothing. |
| `unreadable` | Can't read | Crossed circle | Melomae could not decode this file. The reason is shown on the row. |

The "Already full bandwidth" label avoids the word "lossless" in a claim about restoration. SRS §1.4 bans that claim in `docs/melomae-srs.md:14`. The label describes the source file, not the output. This removes the ambiguity.

The third signature is the **approximation note**. It is a small, quiet label, "Generated detail", next to the restored side in the queue and the compare player. It opens a two-sentence explanation on focus or click. The full caveat appears in first run and in About, as SRS §1.4 requires (`docs/melomae-srs.md:14`). The note keeps the caveat present without a warning banner on every screen.

The interface does not use album art as a hero image, waveform decoration, equalizer clichés, or "AI" language. The product is about measured audio data. The file name, the format, the cutoff, and the result carry the screen.

## 2. Frame and layout

The app is one window with a narrow left rail and one main work area. There are no tabs. The rail has four destinations and a footer:

1. **Restore**. The home screen. Add folders, review the triage, start a batch.
2. **Queue**. Active and recent jobs, with a count badge.
3. **History**. The restore history from `docs/melomae-srs.md:89`.
4. **Settings**. Output, restore, playback, appearance, and About.

The rail footer shows the device state (Auto, CPU, or GPU) and the model state (Ready or Missing). This is a direct read of SRS §6.9 (`docs/melomae-srs.md:69`) and the model file in SRS §11.7 (`docs/melomae-srs.md:123-128`). It is not a network indicator, because the app needs no network (`docs/melomae-srs.md:32`).

Sizes:

| Item | Value | Why |
|---|---|---|
| Default window | 1100 x 720 | The triage table needs six columns and a source pane. |
| Minimum window | 800 x 600 | This is the current configured size in `src-tauri/tauri.conf.json:13-18`. The layout must work at it. |
| Rail | 216 px, collapses to 64 px below 960 px | Labels stay visible at the default size. Icon-only mode keeps an accessible name and a tooltip. |
| Main content | Fluid, max 1280 px | Long tables stay readable on a large display. |
| Inspector drawer | 320 px, overlays below 960 px | File details do not steal table width on a small window. |
| Table row | 36 px, 44 px hit area on controls | A library can have thousands of rows. Controls stay easy to hit. |

The current window is 800 x 600 with no minimum (`src-tauri/tauri.conf.json:13-18`). This proposal keeps 800 x 600 as the floor and sets 1100 x 720 as the default. The frame must pass the resize matrix in task 19.

The visual system is a warm neutral canvas, not a blue dashboard. Starting tokens:

| Token | Light | Dark | Use |
|---|---|---|---|
| `canvas` | `#F4F1EC` | `#171614` | App background |
| `surface` | `#FBF9F6` | `#201E1B` | Panels and rows |
| `ink` | `#1C1A17` | `#F2EDE6` | Primary text |
| `ink-muted` | `#5E5A54` | `#B3ACA2` | Secondary text |
| `border` | `#D8D2C8` | `#3A3732` | Dividers and control edges |
| `accent` | `#9A4708` | `#E9A66B` | Primary action and selection |
| `cutoff` | `#0F6E7A` | `#7FD6DE` | Cutoff marker and measured scale |
| `ok` | `#2F6B4F` | `#7FB694` | Done and restored |
| `warn` | `#8A5A00` | `#D9A441` | Little to gain and skipped |
| `danger` | `#A23B2A` | `#E08374` | Failed and unreadable |

The text pairs above compute to at least 4.5:1 on their stated backgrounds from the sRGB values. I did not take a pixel reading. A second computation on 2026-11-11 confirmed this: the lowest text pair is `warn` on the light canvas at 5.26:1.

The `border` token does not meet this proposal's own rule. It gives 1.33:1 to 1.53:1 on its backgrounds, and section 8 requires 3:1 for a control edge. Use `border` for dividers only. Task 39 must add a separate control-edge token that reaches 3:1. The final values belong in `DESIGN.md` and need the task 19 contrast checks.

Typography: a humanist sans with tabular numerals for the UI, and a mono for paths, hashes, and the provenance string. The starting choice is IBM Plex Sans and IBM Plex Mono, bundled as woff2 and listed in the notices. The fallback is Segoe UI on Windows and Cantarell on Linux. The exact family is a `DESIGN.md` decision, not a runtime web request. SRS §3.4 requires offline operation (`docs/melomae-srs.md:32`), so no font CDN is allowed.

Density: the table is compact by default because a library is the main content. A Comfortable density option increases the row to 44 px. Settings stores the choice.

Motion: 140 ms ease-out for a panel or step change, 80 ms for hover and press. There is no crossfade on the A/B audio switch, because the comparison must be immediate (`docs/melomae-srs.md:102`). Reduced motion removes transforms and keeps opacity and color changes. The motion tokens live in `DESIGN.md` and follow task 19.

## 3. Screens

### 3.1 First run (task 25)

A full-window wizard with a three-step header: "1 What this does", "2 Where to save", "3 How fast". The header is a progress list, not a modal. Focus moves to each step heading and the step change is announced once (task 25).

**Step 1, the caveat.** Two columns. The left column is the caveat in plain language with the cutoff meter as a static illustration. The right column is a short "What stays the same" list: tags, pictures, folder tree, offline. The primary action is Continue. There is no Skip for this step, because SRS §3.3.1 requires it (`docs/melomae-srs.md:29`).

**Step 2, the output folder.** A folder field, a Pick folder button, and a suggested default. The panel shows a path preview for a sample input so the user sees the mirrored tree (`docs/melomae-srs.md:82`). The field validates on pick and on Continue (task 41). The secondary action is Back, not Skip, because a default output folder is required (`docs/melomae-srs.md:30`).

**Step 3, the benchmark.** A live readout: "Measuring this machine", a progress bar, the elapsed time, and the result. The result is "A 4-minute track will take about 12 minutes on the CPU." A GPU line appears only when a GPU exists (`docs/melomae-srs.md:31`, `docs/melomae-srs.md:69`). The actions are Skip and, on failure, Retry. A failed benchmark never blocks the flow (task 25).

**Done.** The primary action is "Add your first folder". The secondary action is "Open Settings". If the user closes the window, the next launch resumes at the first incomplete step. The app opens Restore with a first-run banner when the flow is incomplete.

**About** lives in Settings, not in first run. It holds the Apollo credit, the license name and link, the source link, the modifications, the no-endorsement statement, the GPL-3.0-only license, and the notices link (`docs/melomae-srs.md:112`, `docs/melomae-srs.md:119-120`, `docs/melomae-srs.md:122`). The full caveat repeats here (`docs/melomae-srs.md:14`).

### 3.2 Restore (task 24, the home screen)

A two-pane layout. The left pane is **Sources**, 280 px wide. It lists each added folder with its path, file count, last scan time, and two row actions: Rescan and Remove. The bottom of the pane has Add folder and Add files. The right pane is the **Files** table. Below 960 px the Sources pane collapses into a folder filter above the table.

The screen header is "Restore" with the subtitle "Add folders. Melomae shows which files can gain." The header actions are Add folder (primary) and Add files (secondary). The header stays fixed while the table scrolls.

**Empty state.** A large drop zone fills the table area. It has a quiet cutoff meter illustration, two buttons (Pick folder, Pick files), and one line: "Melomae reads files on this machine. Nothing is uploaded." The drop path must be the real Tauri event path, not a simulated HTML5 drop (task 24). A keyboard user can reach Pick folder without dragging.

**Scanning state.** A progress strip sits above the table: "Scanning 1,204 files. 812 done." It has a Cancel action. Rows stream into the table as triage finishes each file (`docs/melomae-srs.md:36-37`). The table stays usable during the scan. A Rescan reads only changed files (`docs/melomae-srs.md:46`).

**The table.** Columns:

1. Checkbox.
2. Name, with the relative folder path as a secondary line.
3. Verdict, as a visible label plus a shape.
4. Format, as codec and bitrate.
5. Cutoff, as a meter plus a kHz value.
6. Duration.
7. Result, from the last batch, when present.

The header has a select-all checkbox and a per-verdict tri-state checkbox in a filter row. The filter row has five chips with counts. A "Show only unreadable" chip appears when an unreadable file exists. The table sorts by name, verdict, cutoff, or duration.

**The cutoff meter.** A 0 to 22.05 kHz track, 3 px high. A vertical marker shows the measured cutoff. A muted band above the marker shows the lost range. A high-bitrate file usually shows a marker high on the scale and a narrow lost band. A low-bitrate file shows a marker lower on the scale and a wider band. A full-bandwidth file shows a full track with no lost band. The meter is a picture of the measurement, not a claim about the output. The exact marker comes from the task 16 measurement, not from a guess in the UI.

**Selection.** The default follows SRS §4.5 (`docs/melomae-srs.md:45`). Space toggles the focused row. Shift-click selects a range. Ctrl+A selects every visible row. The per-verdict header selects or clears all rows with that verdict. A sticky selection bar at the bottom shows "N of M selected", the output format, the restore state, and Start restore.

The restore state in the bar is a per-batch control: "Restore: On" or "Restore: Audio only". SRS §6.7 allows this per job or by default (`docs/melomae-srs.md:67`). The default lives in Settings. "Audio only" encodes the source and keeps its tags without inference.

**Estimate notice.** A short line above the table: "These verdicts are estimates. You decide which files to restore." A "What do these mean?" disclosure explains all five labels (`docs/melomae-srs.md:44`). The notice is not a modal and does not repeat on every scan after the first.

**Row inspector.** Enter or a row click opens a 320 px drawer. It shows the full path, container, codec, bitrate, sample rate, bit depth, duration, cutoff, tags summary, and picture count. The current list is virtualized with TanStack Virtual per Stack §A (`docs/melomae-stack.md:13`), so the drawer is the place for details, not a second row.

**Unreadable rows.** The row shows "Can't read" and the decoder reason inline (`docs/melomae-srs.md:43`). The row is not selectable. A filter chip shows only unreadable files. The reason has a plain summary and a copyable detail.

**Batch report.** After a batch, a summary strip appears above the table: Restored, Skipped, Failed, and Missing fields, with an "Open report" action. The report is a right-side panel with a per-file table and a "Copy report" action (`docs/melomae-srs.md:97`). Skipped rows say why, such as "An output already exists" (`docs/melomae-srs.md:84`). Failed rows keep the batch running (`docs/melomae-srs.md:94`).

**Output folder error.** If the output folder is missing or read-only, the selection bar shows an inline error and a "Fix in Settings" link. Start is disabled until the folder is valid (task 41).

### 3.3 Queue (task 21)

The Queue screen is a single vertical list, grouped by batch. A sticky summary header shows Active, Waiting, Done, Failed, and a "Cancel batch" action while a batch runs (`docs/melomae-srs.md:95`).

**Empty state.** "No jobs yet. Add folders on Restore." with a link to the Restore screen.

**Job row.** Each row has:

- File name and verdict.
- A stage timeline: Reading, Restoring, Writing, Tagging. The current stage is a filled shape, not a color alone. A restore-off job skips Restoring and says so.
- A progress bar with "Chunk 4 of 12" during restore (`docs/melomae-srs.md:65`).
- An ETA, computed from the current job's measured RTF.
- The device chip (CPU or GPU) and a "GPU fell back to CPU" note when that happens (`docs/melomae-srs.md:63`).
- The clamp count when it is not zero (`docs/melomae-srs.md:77`).
- Actions: Cancel, Retry, Open location, Details. A finished job gets Compare.
- A result label: Done, Skipped, Failed, or Cancelled.

**States.** A "Waiting for the engine" row appears if no event arrives for 15 seconds. It has a spinner and a Reconnect action after 30 seconds (task 21). A "Connecting" state appears before the first event. The progress bar is a `role="progressbar"` with a value. A polite live region announces stage changes only, never chunk counts.

**Error details.** A failed row shows a plain sentence first, such as "This file could not be decoded." A Details disclosure holds the technical text and a Copy action. The UI never renders raw Rust error text (task 18).

**Long content.** The list is virtualized. Batch group headers stay sticky. A 500-job stream is the profile target (task 21). The summary header shows totals outside the virtual list.

**Batch report.** A collapsible section at the top of the batch shows the restored, skipped, failed, and cancelled counts. Cancelled is a fourth result, because SRS §9.5 lists only three (`docs/melomae-srs.md:97`). This needs an SRS amendment. The per-file report opens in the inspector drawer.

### 3.4 Compare (task 23)

The compare player opens from a finished job in the Queue or History. It is a full-screen route with a back link to its origin.

**Header.** The track name, the source path, the output path, the output format, the device, and the restore date. The source path is important because the compare reads the original file (`docs/melomae-srs.md:108`).

**Transport.** A play and pause button (Space), a large position slider with elapsed and total time, and the A/B switch. In normal mode the labels are "Source" and "Restored". The switch keeps the exact sample position (`docs/melomae-srs.md:102`). There is no crossfade.

**Volume matching.** A switch, on by default, with the measured offset in dB, such as "-3.2 dB on the restored side" (`docs/melomae-srs.md:104-105`). A short "Why" disclosure explains that the offset affects playback only and never changes a file. The switch is disabled in blind mode with the line "Blind mode uses matched volume" (`docs/melomae-srs.md:107`).

**Measured cutoff view.** A small two-line sketch on a 0 to 22.05 kHz axis. The source line is solid and stops at the measured cutoff. The restored line is dashed and continues above it. A label says "Generated detail" above the cutoff. This is not the backlog spectrogram. It is a static view of the frequency extension, and it keeps the caveat visible.

**Loading state.** A "Preparing audio" state with a progress bar while the two buffers decode and the loudness is measured (task 23). A long file shows a length warning before the expensive work starts (task 23).

**Unavailable state.** If the source is missing or its hash no longer matches the history, the transport is replaced with a plain explanation and two actions: Locate source and Open history (`docs/melomae-srs.md:108`).

**Blind mode (task 32).** A trial strip: "Trial 3. X is playing. Which one is X?" with A and B buttons and keyboard keys. The user does not see which version is which. The gain offset is unmounted, not just hidden (task 32). A "Trial recorded" line confirms the choice, then the next trial starts. The user can end the session at any time. The result panel shows the correct count, the trial count, the p-value, and a plain sentence. A zero-trial session shows "No trials completed" and no p-value (`docs/melomae-srs.md:106`).

### 3.5 History

A table with Date, File, Verdict, Format, Device, Result, and a Compare action. Filters cover date range, result, verdict, and format. Search filters by file name. The rows come from the history record in `docs/melomae-srs.md:89`.

The inspector drawer shows the input path, output path, input SHA-256, settings, result, and the provenance string in the mono face. The provenance string is a single line of `key=value` pairs (`docs/melomae-srs.md:87`). It has a Copy action.

The Compare action is disabled when the source file is missing or changed, with the reason from SRS §10.8 (`docs/melomae-srs.md:108`). The empty state is "No restores yet. Start on Restore." The table is virtualized for large histories.

### 3.6 Settings

Six sections, each a titled group with a one-line description:

1. **Output.** Default output folder, output format (FLAC, WAV, Opus), and a read-only note that the folder tree mirrors the input (`docs/melomae-srs.md:82`). FLAC is the default (`docs/melomae-srs.md:74`).
2. **Restore.** Restore by default (on or off) (`docs/melomae-srs.md:67`), device (Auto, CPU, GPU) (`docs/melomae-srs.md:69`), and a read-only note that chunks are sized automatically (`docs/melomae-srs.md:64`).
3. **Playback.** Volume matching default (on), stored for the compare player (`docs/melomae-srs.md:105`).
4. **Appearance.** Theme (System, Light, Dark) and density (Comfortable, Compact). System is the default. This is an addition to task 19, which only says "follow the OS".
5. **About.** Version, model hash, Apollo credit, license, notices, and the caveat (`docs/melomae-srs.md:112`, `docs/melomae-srs.md:119-120`, `docs/melomae-srs.md:122`).
6. **Diagnostics.** Model state, device state, and a "Copy diagnostics" action. This supports the first-run benchmark failure in task 25.

Each field has a label, a hint, and a save state: Idle, Saving, Saved, or Failed. The output folder validates on pick and on save (task 41). No section needs a network.

## 4. Flows

**First run.** Launch, caveat, output folder, benchmark, Done, Restore empty state. The flow resumes at the first incomplete step after a close.

**Primary restore flow.** Add a folder, watch the scan, review the verdicts, adjust the selection, choose the output format and the restore state, start the batch, watch the queue, open the batch report, open Compare, then find the row in History.

**Cancel one job.** Cancel on the row. The job stops within one chunk (`docs/melomae-srs.md:66`). The row moves to Cancelled. The batch continues (`docs/melomae-srs.md:94`).

**Cancel the batch.** Cancel batch in the summary header. Each active job stops within one chunk. The report lists the cancelled jobs separately.

**Retry a failed job.** Retry on the row. The job returns to Waiting. The batch report updates.

**Compare the source and the restored file.** Open a finished job, prepare the buffers, play, switch A and B at the same position, adjust volume matching, seek, pause.

**Run a blind session.** Open a finished job, start blind mode, hear X, pick A or B, repeat, end the session, read the result.

**Compare unavailable.** The source moved or changed. The player shows why and links to History. The user can locate the source and try again (`docs/melomae-srs.md:108`).

**Crash recovery.** On launch, the Queue reads the last known job state from the database and marks any job that was running as interrupted. The user can retry. This is grounded in the history record in `docs/melomae-srs.md:89` and the job store in task 14.

## 5. States matrix

| Screen | Empty | Loading | Error | Long content | Other |
|---|---|---|---|---|---|
| First run | Not applicable | Benchmark progress | Benchmark failed with Retry and Skip | Not applicable | Resume at first incomplete step |
| Restore | Drop zone and two add actions | Scan count, progress, Cancel | Unreadable rows and invalid output folder | Virtualized table and inspector | Estimate notice, selection bar |
| Queue | "No jobs yet" | Connecting, Waiting for engine | Plain message, Details, Retry | Virtualized groups, 500+ jobs | Stalled channel, GPU fallback |
| Compare | No job selected | Preparing audio | Decode failed, Retry | Length warning over the limit | Unavailable source, blind mode |
| History | "No restores yet" | Reading history | Read failed, Retry | Virtualized table | Compare disabled with reason |
| Settings | Not applicable | Saving | Folder invalid, save failed | Scrolls in sections | Saved state, diagnostics |

## 6. Component inventory

- `AppFrame`: rail, content, skip link, error boundary.
- `SidebarNav`: four destinations, `aria-current`, device and model footer.
- `ScreenHeader`: title, subtitle, primary and secondary actions.
- `FolderSourceList`: folder rows, Rescan, Remove, Add.
- `TriageTable`: virtualized grid with a header, filter chips, and sort.
- `VerdictChip`: label, shape, and explanation.
- `CutoffMeter`: 0 to 22.05 kHz scale, marker, lost band.
- `SelectionBar`: count, format, restore state, Start, inline error.
- `InspectorDrawer`: file details, tags summary, provenance.
- `JobRow`: stage timeline, progress, ETA, actions, result.
- `ProgressBar`: progressbar semantics, indeterminate mode.
- `BatchReport`: counts, per-file table, copy.
- `CompareTransport`: play, pause, position, A/B.
- `PositionSlider`: slider semantics, keyboard seek, time text.
- `GainOffset`: dB readout and why disclosure.
- `CutoffView`: source and restored frequency sketch.
- `TrialStrip`: blind trial, A/B choice, trial count.
- `ResultPanel`: correct count, p-value, plain sentence.
- `SettingsSection`, `Field`, `SaveState`.
- `EmptyState`, `ErrorState`, `ConfirmDialog`.
- `StatusChip`: text and shape, never color alone.
- `TokenLayer`: CSS custom properties for color, type, space, radius, focus, motion.

## 7. Design decisions

| Decision | Choice | Why | Grounding |
|---|---|---|---|
| Shell | One window and a rail, no tabs | The app has four destinations and one linear flow. A rail keeps the primary action visible. | SRS §3 to §10 |
| Home | Restore is the home screen | The user's main job is to add folders and start a batch. | `docs/melomae-srs.md:93` |
| Signature | Cutoff meter in the row and the player | The cutoff is the product's core measurement. A full spectrogram is backlog. | `docs/melomae-srs.md:37-43`, B-003 |
| Verdict labels | Plain labels, shapes, and explanations | The user is non-technical and must decide per file. | `docs/melomae-srs.md:26`, `docs/melomae-srs.md:44-45` |
| Color | Text and shape carry status, color is secondary | Color alone fails WCAG 1.4.1. | task 19 |
| Selection | Sticky bar with count, format, restore state, Start | The primary action stays visible in a long list. | `docs/melomae-srs.md:93` |
| Audio | Native Rust playback, React only for controls | WebKitGTK audio support depends on installed plugins. | `docs/melomae-stack.md:77-79` |
| A/B switch | No crossfade | The comparison must be immediate at the same position. | `docs/melomae-srs.md:102` |
| Blind mode | Unmount identity and offset | A hidden label still reaches a screen reader. | `docs/melomae-srs.md:106`, task 32 |
| Lists | Virtualize the table and the queue | A library and a batch can hold thousands of rows. | `docs/melomae-stack.md:13` |
| Fonts | Bundle an OFL family as woff2 | The app must work offline and the current Inter name never loads. | `docs/melomae-srs.md:32`, R-001 |
| Theme | System by default, Light and Dark available | Task 19 asks for OS follow. A manual override costs little and helps a user who wants a fixed theme. | `tasks/mvp/19-...md:19` |
| Album art | Not used as a hero or a browsing surface | The product is about audio data, not a media library. | SRS §1, §4 |
| Density | Compact table by default | A music library is the main content. Comfortable is a stored option. | SRS §4.1 |
| Error copy | Plain sentence first, detail behind a disclosure | Rust errors are not user copy. | task 18 |

## 8. Keyboard and accessibility contract

| Context | Keys |
|---|---|
| Global | Tab and Shift+Tab move focus. Escape closes a drawer or dialog. The first Tab reaches Skip to content. |
| Sidebar | Enter or Space opens the focused destination. The active item has `aria-current="page"`. |
| Restore table | Up and Down move the row. Space toggles the row. Shift+Up and Shift+Down extend the range. Ctrl+A selects all. Enter opens the inspector. Delete removes a source folder after a confirmation. |
| Selection bar | Enter on Start starts the batch. The bar announces the selected count on change. |
| Queue | Up and Down move the row. Enter opens Details. C cancels the focused job after a confirmation. R retries. |
| Compare | Space plays or pauses. Left and Right seek 5 seconds. Shift+Left and Shift+Right seek 30 seconds. A plays the source. B plays the restored file. V toggles volume matching outside blind mode. M starts or ends a blind session. In blind mode, A and B answer the trial. |
| Dialogs | Escape cancels. Enter confirms the primary action. Focus stays inside the dialog and returns to the trigger. |

The table is a `role="grid"` with `aria-rowcount` and `aria-rowindex` because it is virtualized. The progress bar uses `role="progressbar"`. Stage changes use a polite live region. The inspector is a `complementary` landmark with a heading. The skip link is the first focusable element. Focus is visible on every control, with a 2 px ring and a 2 px offset (task 19). Text meets 4.5:1. Control edges, focus rings, and progress bars meet 3:1 (task 19). The layout passes 800 x 600 and 200% zoom (task 19).

## 9. Copy deck

These strings are proposals. They obey SRS §1.4 and §1.5 (`docs/melomae-srs.md:14-15`).

- **Caveat, first run and About:** "Melomae rebuilds the high frequencies that lossy compression removed. It generates new detail from patterns in its training data. The result is a plausible reconstruction, not the original recording."
- **Empty Restore:** "Drop a folder here, or pick one. Melomae reads it on this machine and never uploads it."
- **Estimate notice:** "These verdicts are estimates. You decide which files to restore."
- **Can restore:** "This file lost high frequencies. Melomae can add plausible detail."
- **Little to gain:** "This file kept most of its high frequencies. New detail may not help. You can still restore it."
- **Looks converted:** "This file is marked as a full-quality format, but its high frequencies look like a converted file. Check the source."
- **Already full bandwidth:** "This file kept its full frequency range. Restoring would add generated detail to audio that lost nothing."
- **Can't read:** "Melomae could not decode this file. The reason is shown below."
- **Queue empty:** "No jobs yet. Add folders on Restore."
- **Queue stalled:** "Waiting for the engine. If this lasts more than 30 seconds, reconnect."
- **Queue failed:** "This file failed. The batch continued."
- **Queue skipped:** "Skipped. An output already exists at this path."
- **Queue cancelled:** "Cancelled. No partial output was kept."
- **Compare loading:** "Preparing audio. This can take a moment for a long track."
- **Compare unavailable:** "The source file moved or changed. Compare needs the original file to match the history."
- **Blind result, unlikely:** "9 of 10 correct. A result this good by guessing alone is unlikely (p = 0.011)."
- **Blind result, consistent with guessing:** "5 of 10 correct. This result is consistent with guessing (p = 0.623)."
- **Blind result, zero trials:** "No trials completed. Run at least one trial to see a result."
- **Benchmark:** "Measuring this machine. A 4-minute track will take about 12 minutes on the CPU."
- **Folder error:** "Melomae cannot write to this folder. Choose another folder or check its permissions."
- **Generated detail note:** "Generated detail. The restored file contains plausible new high frequencies, not the data the original compression removed."

## 10. How this proposal reaches the board

A proposal has no board row. These records carry its work:

| Part of the proposal | Record |
|---|---|
| Product character, tokens, type, motion (sections 1 and 2) | Task 39 |
| Frame, rail, focus, theme, window size, error boundary | Task 19 |
| Accessibility scans, keyboard runs | Task 40 |
| Settings validation and save states | Task 41 |
| Event envelope and error copy | Task 18 |
| Queue | Task 21 |
| Compare | Task 23 |
| Blind mode | Task 32 |
| Restore and triage | Task 24 |
| First run and About | Task 25 |
| Starter leftovers | R-001 |

The History screen (section 3.5) has no record yet. Task 19 names History in the sidebar, and no task builds its table. Write that record when the owner accepts this proposal.

## 11. SRS gaps this proposal exposes

These need an owner decision before the UI tickets close.

1. **Cancelled jobs are not in the batch report.** SRS §9.5 lists restored, skipped, and failed (`docs/melomae-srs.md:97`). A cancelled job is a fourth result. The proposal shows it separately. The SRS should name it.
2. **First run has no exit or resume rule.** SRS §3.3 gives the order but not what happens if the user closes the window (`docs/melomae-srs.md:28-31`). The proposal resumes at the first incomplete step.
3. **No error taxonomy.** SRS §4.3 requires an unreadable reason (`docs/melomae-srs.md:43`), and SRS §9.2 requires the batch to continue (`docs/melomae-srs.md:94`). No document defines the error codes or the user copy. task 18 proposes a typed error.
4. **No compare length or memory limit.** Task 23 holds both signals as buffers (`tasks/mvp/23-...md:23`), and SRS §10 sets no length bound (`docs/melomae-srs.md:101-104`). task 23 proposes a limit.
5. **No keyboard or empty-state requirement.** No UI ticket names a keyboard map or an empty state. This proposal supplies both, and tasks 18 to 25, 32, and 39 to 41 ask for the tests.
6. **Theme override.** Task 19 says follow the OS (`tasks/mvp/19-...md:19`). The proposal adds System, Light, and Dark. This is an addition, not a conflict. The owner should confirm it.
7. **The word "lossless" in the verdict.** SRS §4.3 names a `lossless` verdict (`docs/melomae-srs.md:42`), and SRS §1.4 bans "lossless" as a restoration claim (`docs/melomae-srs.md:14`). The proposal separates the two with the visible label "Already full bandwidth". The owner should confirm the label.
8. **A skipped benchmark.** SRS §3.3 says that the first-run flow measures the restore speed (`docs/melomae-srs.md:31`). This proposal lets the user skip the measurement, and it lets a failed measurement pass. The SRS states neither. The owner should confirm both, or task 25 keeps the measurement mandatory and adds only the failure path.

## 12. What this proposal does not do

- It does not add a spectrogram. B-003 owns that, and task 23 excludes it (`tasks/mvp/23-...md:32`).
- It does not add a player for the queue before a job finishes.
- It does not browse or edit tags. SRS §5.6 forbids an online lookup, and the product does not add metadata (`docs/melomae-srs.md:56`).
- It does not add album art browsing, streaming, or download links. SRS §1.5 forbids them (`docs/melomae-srs.md:15`).
- It does not define a mobile layout. The product is desktop-only (`docs/melomae-srs.md:22`).
- It does not replace the MVP tickets. It gives each one the interface detail it is missing.
