# B-050 — Write repeated WOAR frames in WAV

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 15

## Impact

lofty 0.25.4 cannot hold two `WOAR` frames in one ID3 tag: `UrlLinkFrame` equality and hash use only the frame ID (`src/id3/v2/items/url_link_frame.rs:18-28`), and `FrameList::push` is `pub(crate)` (`src/id3/v2/frame/list.rs:264`). WAV keeps the first website URL and lists WEBSITE. Reading repeated `WOAR` frames from an MP3 also collapses them. Review 2 of task 15 also found that override matching on read collapses distinct custom names (finding 3).

## Why this does not block MVP

The lost URLs are listed.

## Expected behavior

Every URL survives in WAV, through a lofty change or a raw frame writer, and the reader keeps repeated `WOAR` frames and distinct custom names.

## Promotion condition

Promote this task when lofty can hold repeated URL frames.

## Acceptance criteria

- [ ] A source with two URLs keeps both in WAV.
