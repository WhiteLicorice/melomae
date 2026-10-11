# B-023 — Reject unrepresentable chunk settings

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 04

## Impact

`resolve_chunking` in `crates/melomae-core/src/inference/mod.rs` converts seconds to samples with a saturating float cast. An extreme setting such as an overlap of 1e300 seconds reaches `usize::MAX`, and the next multiplication overflows. A debug build panics. The stock `inference.py` rejects the same setting with Python integer arithmetic. The first task 04 review found this (N3).

## Why this does not block MVP

No ordinary setting reaches the overflow. Task 05 sizes chunks automatically from free memory.

## Expected behavior

Chunk, overlap, and pad settings whose sample counts cannot be represented return a typed error. The padded sizes use checked arithmetic.

## Promotion condition

Promote this task when the app exposes chunk settings to the user, or with the next change to `resolve_chunking`.

## Acceptance criteria

- [ ] A test with an extreme overlap returns a typed error, and fails before the change.
- [ ] Every duration-derived addition and multiplication in the chunking code is checked.
