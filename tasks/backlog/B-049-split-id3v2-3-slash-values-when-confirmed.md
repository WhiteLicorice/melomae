# B-049 — Split ID3v2.3 slash values when confirmed

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 15

## Impact

The author chose option 1 on 2026-10-11: ID3v2.3 slash text stays one value, so a name such as AC/DC is safe. Option 3 splits the text only when another field in the same file, such as the MusicBrainz artist IDs, confirms the count.

## Why this does not block MVP

Option 1 never changes the user's text.

## Expected behavior

The copier splits v2.3 slash text only when another field confirms the count, with tests for AC/DC and for a confirmed list.

## Promotion condition

Promote this task when a user asks for split artists from v2.3 files.

## Acceptance criteria

- [ ] Both tests pass.
