# B-042 — Compare complete pictures in the tag write check

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 15

## Impact

The write check in `crates/melomae-core/src/tag_copy/mod.rs` compares portable text values only. Pictures and carried frames are outside it. A FLAC source with two equal front pictures gives one picture in WAV, because lofty merges equal picture frames (`lofty-0.25.4/src/id3/v2/tag.rs:969`), and the returned list does not name it. Review 6 of task 15 found this (finding 3).

## Why this does not block MVP

The normal two-picture and large-picture cases pass, and the duplicate case is constructed.

## Expected behavior

The check compares pictures as a multiset of bytes, type, description, and MIME type, and lists a lost picture under a documented name.

## Promotion condition

Promote this task with the next change to picture handling.

## Acceptance criteria

- [ ] A test with two equal pictures sees the loss listed for WAV.
