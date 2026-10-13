# B-036 — Label GPL license files by their real version

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 26

## Impact

`tools/generate_notices.py:319` labels any license file whose tokens contain `GPL` as `GPL-3.0-or-later`. A `LICENSE-GPL-2.0` or a `GPL-3.0-only` file gets the wrong label. The document body stays correct, and no collected file reaches this branch today. Review 3 of task 26 found this (N2).

## Why this does not block MVP

No component in the current graph has a GPL license file, and the label does not change the deny policy.

## Expected behavior

The label comes from the crate's declared license expression, or the file stays unlabelled. Tests cover a GPL-2.0-only and a GPL-3.0-only input.

## Promotion condition

Promote this task when a dependency with a GPL license file enters the graph.

## Acceptance criteria

- [ ] Both tests pass, and no GPL file is labelled with a version that its crate does not declare.
