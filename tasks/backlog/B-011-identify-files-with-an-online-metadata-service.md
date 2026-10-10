# B-011 — Identify files with an online metadata service

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 15

## Impact

Some users have untagged or badly tagged files. MusicBrainz lookup or AcoustID fingerprints could fix their tags.

## Why this does not block MVP

Collectors' files already carry tags. SRS §5 keeps them. The free terms of MusicBrainz and AcoustID cover non-commercial use only (`docs/evidence/2026-10-10-pivot.md`).

## Expected behavior

The user asks for a lookup. Melomae proposes tag changes. The user approves each change before Melomae writes it.

## Promotion condition

Promote this task when users ask for tag correction and the owner has a commercial agreement with MetaBrainz and AcoustID, or the project has no planned revenue.

## Acceptance criteria

- [ ] The owner records the service terms that apply, with the date checked.
- [ ] No tag changes without the user's approval.
