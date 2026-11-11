# 43 — Copy tags and pictures on the CLI restore path

**Status:** TODO
**Phase:** 2 — Metadata, triage, and provenance
**Depends on:** 07, 15
**SRS:** §5
**Stack:** §G
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Make a command-line restore keep the tags and the pictures of its source, and report each field that the output format cannot hold.

## Evidence

The ponytail audit of 2026-11-11 read the code at `master` `23f0bcf`. It built and ran nothing. The orchestrator read each cited line again on the same day.

- `crates/melomae-cli/src/restore.rs:224` calls `encode_with_cancel`.
- `crates/melomae-core/src/encode/mod.rs:203-210`: `encode_with_cancel` passes `None` for the tags.
- `crates/melomae-core/src/encode/mod.rs:183-189`: `encode_with_tags` takes the tags, and it uses `NEVER_CANCELLED`. No entry takes the tags and a cancel flag together.
- A search of `crates` and `src-tauri` found no caller of `encode_with_tags` outside `crates/melomae-core/tests`.
- Task 07 put tags out of scope on purpose, and task 15 came later. No record joined the two.
- SRS §5.1 requires every tag to reach the output (`docs/melomae-srs.md:51`). SRS §5.5 forbids a silent drop (`docs/melomae-srs.md:55`).

## Scope

1. Add one encode entry that takes the tags and the cancel flag. Task 17 needs the same entry for the app pipeline.
2. In the CLI, read the source tags with `tag_copy::read_tags` and pass them to that entry.
3. Print each field that the output format cannot hold on stderr.
4. A source with unreadable tags fails with a typed message. It does not write an untagged output.

## Out of scope

- The job pipeline of the app (task 17)
- Provenance tags (task 31)

## Test-first plan

1. A CLI test restores the task 15 tag fixture to FLAC and reads the standard fields, one custom field, and the front cover from the output.
2. A CLI test restores a source with a field that WAV cannot hold, and finds the field name on stderr.
3. A CLI test cancels during the encode and finds no output file.

Run the tests. They fail because the CLI writes no tags.

## Acceptance criteria

- [ ] A CLI restore of the tag fixture keeps its standard fields, its custom fields, and its front cover.
- [ ] The run prints each field that the output format could not hold.
- [ ] The new tests failed first for the expected reason, or `Outcome` says why not.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-cli
pnpm verify
```
