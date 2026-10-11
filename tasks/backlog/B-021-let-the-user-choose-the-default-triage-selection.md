# B-021 — Let the user choose the default triage selection

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 24

## Impact

SRS §4.5 fixes the default selection on the triage screen: `restore`, `suspect-lossless`, and `low-benefit` files start selected, and `lossless` files start cleared. The owner changed `low-benefit` to selected on 2026-10-11, because the verdict is an estimate and the user decides. A user who wants a different default must change the selection on every scan.

## Why this does not block MVP

The user can select or clear any file, or every file with one verdict (task 24). The default saves clicks only.

## Expected behavior

A setting lists the verdicts and lets the user choose which ones start selected. The SRS §4.5 default applies until the user changes it.

## Promotion condition

Promote this task when users report that the default selection is wrong for them.

## Acceptance criteria

- [ ] A setting stores the default-selected verdicts in the task 14 store.
- [ ] The triage screen applies the setting on each scan.
- [ ] A test proves each verdict can be turned on and off by default.
