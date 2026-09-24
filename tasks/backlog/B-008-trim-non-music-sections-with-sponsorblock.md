# B-008 — Trim non-music sections with SponsorBlock

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 02, 12, 13

## Impact

Some tracks exist on YouTube only as a music video. A music video can carry a skit intro, an outro, or other audio that is not in the release. The duration gate (SRS §5.2) rejects that candidate and sends the track to review. The user must then decide by hand.

## Evidence

Checked on 2026-09-24:

- SponsorBlock has the category "Music: Non-Music Section" (`music_offtopic`). The guidelines say: "Only to be used on videos which feature music as the primary content." Segments "should only include music not present in the official or Spotify music release." (SponsorBlock wiki, Guidelines.)
- yt-dlp can remove SponsorBlock segments with `--sponsorblock-remove`. This is a post-processing step, and the yt-dlp README says ffmpeg is "Required for … various post-processing tasks." Melomae ships without ffmpeg.
- "The API and database follow CC BY-NC-SA 4.0 unless you have explicit permission." (SponsorBlock wiki, Database-and-API-License.)
- Each lookup sends the video ID to the SponsorBlock API, a third-party service.

## Why this does not block MVP

- The matcher prefers song results from Topic channels. They carry the release audio and have no SponsorBlock segments. This is inferred. Spike 02 measures it.
- The duration gate stops a music video with extra sections from reaching the library. The worst case is extra review work, not wrong audio.

## Expected behavior

1. An opt-in setting turns the feature on. It is off by default.
2. For a candidate that fails the duration gate and is a music video, the app fetches its `music_offtopic` segments from the SponsorBlock API.
3. After decoding, the app cuts the segments in Rust with sample accuracy. It does not use ffmpeg.
4. The app runs the duration gate again on the trimmed length.
5. The settings screen and the About screen show the SponsorBlock attribution and the CC BY-NC-SA 4.0 notice. The setting states that the lookup sends video IDs to a third party.

## Promotion condition

Promote this task when spike 02 or task 12 data shows that at least 5% of tracks have only a music-video candidate that fails the duration gate. The 5% threshold is inferred. The owner can change it.

## Acceptance criteria

- [ ] A fixture music video with a known non-music intro passes the duration gate after the trim.
- [ ] The cut is sample-accurate against the segment times.
- [ ] With the setting off, the app makes no SponsorBlock request.
- [ ] The attribution and the license notice appear.
