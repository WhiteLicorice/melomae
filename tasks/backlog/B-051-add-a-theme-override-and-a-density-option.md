# B-051 — Add a theme override and a density option

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 19

## Impact

The app follows the OS theme and has one table density. A user who wants a fixed theme, or larger rows, has no setting for it.

## Why this does not block MVP

Task 19 follows the OS theme, and the tokens pass the contrast check in both themes. One density is enough to restore a library.

## Expected behavior

Settings has an Appearance group: Theme (System, Light, Dark) and Density (Compact, Comfortable). The app stores both values. `tasks/ux/interface-proposal.md` section 3.6 holds the proposal.

## Promotion condition

Promote this task when task 19 records that Linux WebKitGTK does not report the OS theme, or when a user asks for either setting.

## Acceptance criteria

- [ ] Each theme value applies at once and survives a restart.
- [ ] Each density value applies to the triage table, the queue, and the history table.
- [ ] The contrast test of task 39 passes for each theme.
