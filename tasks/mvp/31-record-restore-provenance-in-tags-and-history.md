# 31 — Record restore provenance in tags and history

**Status:** TODO
**Phase:** 2 — Metadata, triage, and provenance
**Depends on:** 14, 15
**SRS:** §8
**Stack:** §F, §G
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Let a user and the app tell how each output was made.

## Scope

1. Write the provenance field in SRS §8.5 and §8.6 into every output.
2. Keep earlier `MELOMAE_PROVENANCE` values from the source. Add the new value as another value of the same field (SRS §8.7).
3. Never change any other field.
4. Write the history row in SRS §8.8. Compute the input SHA-256 while the input is read for decode, so the file is not read twice.

## Out of scope

- The history screen (task 19 adds the sidebar entry, task 23 uses the history)

## Test-first plan

1. For FLAC, WAV, and Opus, the provenance value reads back and parses into the six keys.
2. A source with one earlier provenance value gives an output with two values.
3. A source with a user comment keeps the comment unchanged.
4. A history row holds the input SHA-256 that matches a separate hash of the input.

Run the tests. They fail because the writer does not exist.

## Acceptance criteria

- [ ] All four tests pass.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core provenance
pnpm verify
```
