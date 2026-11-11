# R-005 — Remove the unused melomae_core::VERSION constant

**Status:** TODO
**Severity:** Refactor
**Depends on:** 00

## Impact

The core crate exports a `VERSION` constant that nothing reads. Two other places read the package version in a different way.

## Evidence

The ponytail audit of 2026-11-11 read the code at `master` `23f0bcf`. It built and ran nothing. The orchestrator read each cited line again on the same day.

- `crates/melomae-core/src/lib.rs:15`: `pub const VERSION: &str = env!("CARGO_PKG_VERSION")`.
- A search of `crates` and `src-tauri/src` on 2026-11-11 found no other use of `VERSION`, apart from `SCHEMA_VERSION`.
- The audit reports that `encode/opus.rs:123` and `tag_copy/mod.rs:1294` call `env!("CARGO_PKG_VERSION")` directly.

## Why this is a refactor

Nothing fails. The change removes one dead public name, or gives the version one source.

## Expected behavior

The two call sites use `VERSION`, or the constant is deleted. Prefer the first option, because task 31 writes the version into the provenance value.

## Promotion condition

Promote this task with task 31.

## Smallest correct fix boundary

- Do not change the version string that reaches a file.

## Test-first plan

- The gate is the characterization test. The Opus vendor string and the tag tests must pass with no change.

## Acceptance criteria

- [ ] A search finds one way to read the package version in the core crate.
- [ ] `cargo clippy --workspace -- -D warnings` passes.
