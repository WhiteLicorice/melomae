# B-052 — Add a sources pane and a file inspector to the Restore screen

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 24

## Impact

The Restore screen of task 24 is one table. The user cannot see which folders they added, remove one folder, scan one folder again, sort the table, or read every field of one file.

## Why this does not block MVP

Task 24 lets the user add files and folders, review each verdict, select files, and start the batch. A row shows the full file name on focus and the reason for an unreadable file.

## Expected behavior

`tasks/ux/interface-proposal.md` section 3.2 holds the proposal:

- A Sources pane lists each added folder with its file count, a Rescan action, and a Remove action.
- The table sorts by name, verdict, cutoff, or duration.
- A drawer shows the full path, the container, the codec, the bitrate, the sample rate, the duration, the cutoff, and a tag summary of one file.

## Promotion condition

Promote this task when the owner restores a library from more than one folder and reports that the single table is hard to use.

## Acceptance criteria

- [ ] Remove takes the files of one folder out of the table and leaves the others.
- [ ] Rescan reads only the changed files of that folder (SRS §4.6).
- [ ] The drawer opens from the keyboard and returns the focus to its row.
