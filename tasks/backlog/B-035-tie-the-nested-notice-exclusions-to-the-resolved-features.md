# B-035 — Tie the nested-notice exclusions to the resolved features

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 26

## Impact

`tools/generate_notices.py:144` (`EXCLUDED_NESTED`) excludes nested notices by crate name and path, and `:418` and `:426` apply that list without reading the resolved features. The exclusions are correct today: `libdbus-sys` builds without `vendored`, `libsqlite3-sys` without SQLCipher, and `tracing-core` with `std`. If a later change turns on one of those features, the nested notice stays excluded. Review 3 of task 26 found this (N1).

## Why this does not block MVP

No excluded feature is on in the current graph, so the shipped notices are complete today.

## Expected behavior

Each exclusion records the crate version and the feature condition it relies on. The generator fails, or includes the notice, when the resolved features or the version no longer match. A fixture that turns on the feature proves it.

## Promotion condition

Promote this task when a dependency of `libdbus-sys`, `libsqlite3-sys`, or `tracing-core` changes features or version.

## Acceptance criteria

- [ ] A fixture with the excluded feature on makes the generator include the notice or fail.
