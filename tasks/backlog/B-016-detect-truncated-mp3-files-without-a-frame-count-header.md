# B-016 — Detect truncated MP3 files without a frame-count header

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 03

## Impact

The decoder rejects a truncated file when the container declares more frames than it decodes. An MP3 declares a count only in a Xing, Info, or VBRI header. For an MP3 with no such header, Symphonia estimates the count from the bitrate, so the decoder cannot use it, and a truncated file decodes with no error. The detector also reads only a bounded window after the ID3v2 tags, and it does not validate every Xing, Info, VBRI, or ID3v2 field.

## Why this does not block MVP

Most encoders (LAME, ffmpeg) write an Info or Xing header, and those files are checked. The task 03 rulings accepted this limit.

## Expected behavior

A truncated headerless MP3 returns a typed error when the last frame is incomplete. The detector validates the Xing, Info, and VBRI fields and the first-frame extent, and it handles short reads.

## Promotion condition

Promote this task when a user reports a truncated MP3 that decoded with no error.

## Acceptance criteria

- [ ] A headerless MP3 cut inside its last frame returns a typed error.
- [ ] An intact headerless MP3, with and without trailing tags, still decodes.
- [ ] Tests cover MPEG-1 and MPEG-2, mono and stereo, VBRI, and ID3v2 version, flag, and footer cases.
