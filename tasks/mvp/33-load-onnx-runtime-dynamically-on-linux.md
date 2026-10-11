# 33 — Load ONNX Runtime dynamically for the glibc 2.35 baseline

**Status:** TODO
**Phase:** 1 — Restoration engine
**Depends on:** 04
**SRS:** §2
**Stack:** §C, §J
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Make the inference engine build and run on Linux with glibc 2.35, as SRS §2 requires, and keep Windows working.

## Evidence

- Hosted CI run 38112705871 on `af021d8` (2026-10-11): `windows-latest` passed the whole gate, the parity test included. `ubuntu-22.04` failed to link `melomae-core`. The static ONNX Runtime archive in `libort_sys` references `__isoc23_strtol`, `__isoc23_strtoll`, `__isoc23_strtoull`, and a newer libstdc++ symbol. `__isoc23_*` exists only in glibc 2.38 and later. Ubuntu 22.04 has glibc 2.35.
- `crates/melomae-core/Cargo.toml` pins `ort = "=2.0.0-rc.13"` with its default features, which download and statically link a prebuilt ONNX Runtime.
- SRS §2 targets Linux x86_64 with glibc 2.35 or later. Stack §J builds the Linux packages on Ubuntu 22.04. Stack §J already bundles the ONNX Runtime libraries as installer resources.
- The owner chose to keep the glibc 2.35 baseline and load ONNX Runtime dynamically on 2026-10-11.

## Scope

1. Use the `ort` `load-dynamic` feature (or the equivalent of the pinned `ort` version) so that no prebuilt static ONNX Runtime is linked.
2. Pin one official Microsoft ONNX Runtime release for Linux x86_64 and Windows x86_64 whose API version matches the pinned `ort`. Confirm its glibc requirement from the release or its build configuration, and record it.
3. Add a fetch script, in the pattern of `tools/fetch_ffmpeg.py`, that downloads the pinned release into `.cache/`, checks its SHA-256, and extracts the shared library. `pnpm verify` runs it before the tests, and CI caches it.
4. The engine finds the library from an explicit path given by the caller, with a documented fallback for tests and development. A missing or wrong library returns a typed error, never a panic.
5. Keep every inference test, the parity test included, green on Windows. Make the same suite pass on Ubuntu 22.04.

## Out of scope

- GPU execution providers (task 08)
- The installer bundling (tasks 27 and 28)

## Test-first plan

1. A test that loads the engine from a path that does not exist returns the typed error. It fails before the change because the loading path does not exist.
2. The existing inference and parity tests are the regression suite.
3. Prove the Linux link and run in a real Ubuntu 22.04 environment: the hosted CI job, or a local Ubuntu 22.04 container or WSL distribution after the owner approves any system package install.

## Acceptance criteria

- [ ] Hosted CI passes on `ubuntu-22.04` and `windows-latest` with the parity test running on both.
- [ ] `Outcome` records the ONNX Runtime release, its SHA-256 values, and its glibc requirement.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core inference
pnpm verify
```
