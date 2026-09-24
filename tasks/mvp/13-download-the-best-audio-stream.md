# 13 — Download the best audio stream

**Status:** TODO
**Phase:** 2 — Metadata and acquisition
**Depends on:** 11, 12, 30
**SRS:** §5
**Stack:** §E
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Download the best audio stream for a matched candidate, with progress and cancel, and without re-encoding.

## Evidence

- YouTube serves no MP3. The audio-only choices are Opus/WebM (formats 249, 250, 251) and AAC/M4A (format 140).
- Task 02 recorded how `--fixup never` behaves without ffmpeg. Read its evidence file first.

## Scope

**Source interface (core requirement).** Implement `YouTubeMusicSource::fetch`, `claims_url`, and `candidate_from_url` in `source/youtube/`. The YouTube source must pass the task 30 contract suite. The format selector below stays inside the source. `fetch` returns a `StreamInfo` with the codec, container, bitrate, sample rate, and `lossless = false`.

1. Run yt-dlp with `-f "bestaudio[acodec=opus]/bestaudio[acodec^=mp4a]"`, with no ffmpeg, and with the fixup settings from task 02. Opus comes first. AAC is the only fallback. The selector never picks a muxed video stream, and nothing re-encodes the audio.
2. Record the chosen format ID, codec, and bitrate on the job. Task 21 shows an AAC fallback to the user.
3. Parse progress from a JSON `--progress-template`.
4. On cancel, kill the whole process tree: a Job Object on Windows, a process group on Linux.
5. Retry up to 3 times with backoff.
6. Classify errors as bot check, unavailable, geo-blocked, or network.
7. After the download, check that the file decodes with the task 03 decoder.

## Out of scope

- The queue UI (task 21)

## Test-first plan

1. A fake yt-dlp script emits progress lines and each error class. The parser and the classifier handle them.
2. A cancel leaves no child process.
3. The YouTube source runs the task 30 contract suite against the fake yt-dlp script.

Run the tests. They fail because the downloader does not exist.

## Acceptance criteria

- [ ] One real download on Windows passes. `Outcome` records the format ID and bitrate.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core download
pnpm verify
```
