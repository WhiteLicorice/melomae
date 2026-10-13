# B-043 — Honor the ID3v2.4 extended header in the raw frame count

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 15

## Impact

The raw `WOAR` count (`crates/melomae-core/src/tag_copy/mod.rs:647`, `:759`) starts at byte 10 and does not skip an extended header. An MP3 with a v2.4 extended header and two `WOAR` frames loses one URL in every output with an empty returned list. The same function also reads the whole file into memory to count frames. Review 6 of task 15 found this (finding 4).

## Why this does not block MVP

The extended-header case is constructed, and the header-free case is reported correctly.

## Expected behavior

The walk honors the extended-header flag and size, reads only the tag, and treats an incomplete walk as unknown, not as a complete count.

## Promotion condition

Promote this task when a user file with an extended header appears, or with B-050.

## Acceptance criteria

- [ ] A test with an extended header and two URLs lists WEBSITE.
