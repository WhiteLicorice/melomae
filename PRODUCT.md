# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

The primary user owns a music library and does not work in a terminal. That
person installs Melomae without Python, conda, ffmpeg, or a command line (SRS
§3.1). The records do not describe how the files were made. The product assumes
that the user owns them already and wants the top end back.

The user does not know what a spectral cutoff is. The user knows that a file
sounds thin. The user decides file by file, and needs to know which files can
gain before an hour of computer time goes into a batch (SRS §4.5).

Two smaller audiences exist. A returning user restores a batch and then reads
History. A technically able user reads the provenance string and the About
screen. Neither one is the design target.

## Product Purpose

Melomae restores a music collection that the user already owns, on the user's
own machine (SRS §1). The user adds folders. Melomae measures each file, shows
which files can gain, and restores the selected files with Apollo. It keeps
every tag and picture, and it writes into a folder tree that mirrors the input.

Success is a user who can tell, before a long job, which files will benefit,
and who understands what the result is and what it is not.

## Positioning

The restore runs on the user's machine against the user's own files. Nothing
is uploaded, nothing is downloaded, and the app names no streaming service and
no download tool (SRS §1.5, §3.4). The user sees the reading that drives the
verdict: the measured cutoff, drawn on a 0 to 22.05 kHz scale.

A service that restores audio in the cloud cannot make the same claims. It
cannot promise that no audio leaves the machine, and it cannot show an offline
measurement of a local file.

## Operating Context

- A Tauri desktop app for Windows 10 and 11 and for Linux (SRS §2). The UI runs
  in the system webview, and the engine runs in Rust.
- One window, an offline machine, and a folder tree that the user already has.
- A library can hold thousands of files. A batch can hold hundreds of jobs.
- The engine owns a job and reports progress to the UI. The UI is a view of
  that job.
- The compare player and the History screen read files that stay on disk.

## Capabilities and Constraints

- Input formats are mp3, m4a, wav, aiff, aif, flac, opus, ogg, and webm (SRS
  §4.1).
- Output formats are FLAC, WAV, and Opus. FLAC is the default (SRS §7).
- Each file gets one verdict: can restore, little to gain, looks converted,
  already full bandwidth, or cannot read (SRS §4.3, `tasks/ux/interface-proposal.md`
  §1).
- Tags and pictures are preserved, and the output tree mirrors the input (SRS
  §5, §8).
- The app needs no network access, at first run or later (SRS §3.4).
- The app does not download audio, does not link to a download tool, and does
  not search an online metadata service (SRS §1.5, §5.6).
- The restored audio is a plausible reconstruction. The copy must not say
  "lossless", "original quality", or "recovered" (SRS §1.4).
- A failed file does not stop the batch (SRS §9.2).

## Brand Commitments

The name is Melomae. The app is free and open-source, and the code ships under
GPL-3.0-only (SRS §11.1).

The voice is measured, honest, and local. It reports what the app measured, it
states what the app will generate, and it uses a plain word where a term of art
would confuse. The visible label "Already full bandwidth" replaces
"lossless". The word "AI" does not appear in visible copy.

The project credits the Apollo authors, names CC BY-SA 4.0, states the
modifications, and claims no endorsement (SRS §11.3, §11.4).

Melomae must not look like a streaming service or a media browser. No album
art as a hero image, no browsing by cover, no download button, no
recommendation shelf. A user who mistakes it for a store has read the wrong
thing.

## Evidence on Hand

- `docs/melomae-srs.md` defines behavior. It is the source for every claim in
  this file.
- `docs/melomae-stack.md` defines the implementation and records the measured
  baselines.
- `tasks/ux/interface-proposal.md` holds the accepted interface proposal and
  its MVP ruling of 2026-11-11.
- `docs/evidence/` holds the dated evidence records, including the 2026-10-10
  pivot.
- The repository holds no user research, no testimonials, and no customer
  names. Future work must not invent them.
- The benchmark figures in the interface proposal are examples. A real figure
  comes from the first-run measurement on the user's machine.

## Product Principles

1. Show the reading, then the claim. A verdict reports a recorded cutoff, and
   the app shows the reading beside it.
2. The user decides. Triage selects a default set, and the user can change
   every file (SRS §4.5).
3. Nothing leaves the machine. Every feature works offline, and no screen may
   suggest otherwise.
4. Keep the caveat where the generated detail appears. The full statement
   lives in first run and About, and the "Generated detail" label rides with
   every restored side.
5. Repair one thing and keep the rest. A failed file does not stop the batch,
   and a failed benchmark does not block first run.

## Accessibility & Inclusion

The user is non-technical, and some users cannot see a color difference or use
a mouse. Every state also has a word and a shape (DESIGN.md).

Text reaches 4.5:1. A control edge, a focus ring, and a progress bar reach
3:1. Every ratio holds in the light theme and in the dark theme.

Every action has a keyboard path, the layout works at 800 x 600 and at 200%
zoom, and the app follows the OS theme and the OS reduced-motion setting.
