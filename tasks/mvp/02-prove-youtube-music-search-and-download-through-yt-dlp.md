# 02 — Prove YouTube Music search and download through yt-dlp

**Status:** TODO
**Phase:** 0 — Foundation and go/no-go spikes
**Depends on:** 00
**SRS:** §3, §5, §11
**Stack:** §E
**Assigned to:** Agent
**Started:** —
**Outcome:** —

This task is a spike. It ends with `GO` or `NO-GO`.

## Goal

Prove that the official yt-dlp and Deno binaries can search YouTube Music, return durations, and download an Opus stream without ffmpeg, cookies, or Python.

## Evidence

- yt-dlp needs an external JavaScript runtime for full YouTube support (yt-dlp issue #15012, since 2025.11.12). Deno is the default runtime. The official binaries bundle the EJS component.
- YouTube serves no MP3. The audio-only choices are Opus/WebM (formats 249, 250, 251) and AAC/M4A (format 140).

## Scope

1. Download the current official yt-dlp and Deno binaries for Windows. Verify them against their published SHA-256 sums (yt-dlp `SHA2-256SUMS`, Deno `.sha256sum`).
2. Pick 50 MusicBrainz recordings across genres and eras. Include recordings that have live, remix, and edit variants. Record their MBIDs.
3. For each recording, run a flat YouTube Music search (`https://music.youtube.com/search?q=…` with `-J --flat-playlist`). Record which fields exist, especially duration, channel, and a "Topic" channel marker.
4. For each recording, count the audio formats that exist: 251, 250, and 249 (Opus) and 140 (AAC). Record the Opus bitrate.
5. Download the best stream with PATH stripped of ffmpeg, conda, and Python. Check how `--fixup never` behaves for DASH M4A.
6. Record any bot check or sign-in prompt, and the search latency.
7. Record the binary sizes and licenses. yt-dlp's PyInstaller binaries include GPLv3+ code. Deno is MIT.
8. Record whether QuickJS could replace Deno. QuickJS is disabled by default.
9. Save the scrubbed search JSON as matcher fixtures for task 12. Save metadata only. Never save or commit audio.
10. **Source interface (core requirement).** Record which search result fields map to the neutral `Candidate` type from task 30: source URL, title, artist, duration, `official_audio`, and thumbnail. Name any field that YouTube lacks.
11. For SponsorBlock backlog record B-008, count the sampled tracks whose only candidate is a music video. For those tracks, count how many fail the max(3 s, 2%) duration gate.
12. Write all results to `docs/evidence/02-ytdlp-spike.md`.

## GO criteria

- [ ] At least 90% of the sample returns a downloadable Opus stream without cookies.
- [ ] The flat search returns durations.
- [ ] The AAC-only share is recorded. If it is above 10%, raise it with the owner before `GO`.

## NO-GO path

Set tasks 11, 12, and 13 to `BLOCKED`. Write a board amendment. The named candidate is rustypipe for search. Stop and report to the owner.

## Out of scope

- The binary manager (task 11)
- The matcher (task 12)
- Any committed audio

## Test-first plan

This is a spike. Record the manual commands and their output in the evidence file instead of a test.

## Acceptance criteria

- [ ] `Outcome` records `GO` or `NO-GO` with the measured numbers.
- [ ] The matcher fixtures hold metadata only.
- [ ] `python tasks/validate_board.py` passes.

## Verify

```powershell
python tasks/validate_board.py
```

Run the commands recorded in `docs/evidence/02-ytdlp-spike.md` again.
