# 33 — Load ONNX Runtime dynamically for the glibc 2.35 baseline

**Status:** DONE
**Phase:** 1 — Restoration engine
**Depends on:** 04
**SRS:** §2
**Stack:** §C, §J
**Assigned to:** Agent
**Started:** 2026-10-11 — Base commit ac9a07c, clean tree, `git lfs pull` in the worktree. Baseline gate at ac9a07c: board 57 records, tasks unittest 9 passed, Vitest 1 passed, cargo test 144 passed, pytest 14 passed.
**Outcome:** DONE 2026-10-11. `melomae-core` loads ONNX Runtime at run time and links no prebuilt runtime, so the glibc 2.35 baseline of SRS §2 holds. `ort` is pinned at 2.0.0-rc.13 with `default-features = false` and `features = ["load-dynamic", "api-27"]`. `inference::init_runtime(&Path)` loads the runtime once per process from a caller-given library path. It resolves the library with `std::fs::canonicalize` and keeps one runtime per process. A later call with the same resolved path is a no-op `Ok`. A different path, or a path that does not exist, returns `InferenceError::Runtime` and names the loaded path and the requested path. A path that is not a file returns `InferenceError::Runtime` before ONNX Runtime is touched. A library that fails to load returns `InferenceError::Runtime`, and the failure is cached, so a later call returns the same error and never re-enters ONNX Runtime. Fix round 1 added the cached failure state, so a repeated failed load no longer panics or aborts. A process restart is required to retry a load after a failure. `inference::load_onnx_backend(runtime_library, model)` initializes the runtime and then loads `apollo.onnx`. The app's own resource-path wiring is future work, in tasks 17, 27, and 28. `tools/fetch_onnxruntime.py` pins one Microsoft release and `pnpm verify` runs it before the tests. Release: ONNX Runtime v1.28.3, whose C API matches the pinned `ort` (`ort` v2.0.0 uses ONNX Runtime 1.28.0; `MINOR_VERSION` is 27). SHA-256: `onnxruntime-linux-x64-1.28.3.tgz` `db14e4863bd37893fc59729d986ab2a0d043d10b7d44da1913c4982b7e3d009c` and `onnxruntime-win-x64-1.28.3.zip` `1d6fab48e85f948436af7c8c971d2c145cf224e2c444755dec894f8b0de11a83`. Both hashes matched the GitHub release digests on download. glibc requirement: the newest versioned glibc symbol of `libonnxruntime.so.1.28.3` is `GLIBC_2.27`, read from the ELF version-need table; it holds no `__isoc23_*` symbol and needs libstdc++ up to `GLIBCXX_3.4.21`. Ubuntu 22.04 has glibc 2.35 and a newer libstdc++, so both are satisfied. The pyke.io binary that `ort`'s default features download needs `__isoc23_strtol`, `__isoc23_strtoll`, and `__isoc23_strtoull` (glibc 2.38 and later), which is the link failure the dynamic load removes. Red-first: `cargo test -p melomae-core inference` failed with `unresolved import melomae_core::inference::init_runtime`, a `load_onnx_backend` arity error, and a missing `InferenceError::Runtime` variant before the change. Finding: ONNX Runtime 2.0.0-rc.13 marks its dylib slot as used even when a load fails. Round 0 avoided the second load only by checking `is_file()` first. Fix round 1 replaced the two-state record with not-tried, loaded, and failed states. Fix round 1 red: `cargo test -p melomae-core inference --no-fail-fast` failed. `inference_runtime_retry` aborted with `0xc0000409`, and `inference_runtime_path` returned `Ok` for a different library. Gate: `pnpm verify` exit 0 on the fix-round commit, written to `SCRATCH/evidence-33/verify-round1.txt`. Board 57 records, tasks unittest 9 passed, Vitest 1 passed, cargo test 151 passed, pytest 19 passed locally. The cargo count adds the seven new Rust tests over the round-0 baseline of 144, and the pytest count adds the five new fetch-script tests. The local pytest run has all 19, because the export dependency group is on here. Hosted CI reports pytest 12 passed and 3 skipped on each OS, because the export group is off there. Hosted CI run 38118459700 on `7f5450c` passed on `ubuntu-22.04` and `windows-latest`, the parity test included, which proves the normal Linux link and run for the round-0 head. The round-1 head needs its own hosted run. The orchestrator starts that run after this round, and the hosted acceptance item stays unticked until it is green. Not confirmed here: a genuine older ONNX Runtime on Windows, a local Linux run, and the packaged application's resource-path wiring (tasks 17, 27, and 28). N1, the cached library is not verified again, is non-blocking and the orchestrator files it at merge. Evidence: `SCRATCH/evidence-33/`.

## Merge review

- Executor: `deepseek-v4.1-flash` (high) in CommandCode, a first session and a fresh session for fix round 1. Reviewers: two fresh Codex `gpt-6.1-sol` (medium) runs, adversarial.
- Author rulings: keep the glibc 2.35 baseline and load ONNX Runtime dynamically (2026-10-11). The orchestrator pushed `task/33` to origin to run hosted CI, with the author's approval, and deletes it after the merge.
- Review 1 on `7f5450c`: FAIL. B1: a second failed runtime load aborted the process, because the wrapper stored nothing after a failed `ort::init_from` and ort's once-cell had completed without data. B2: after a successful load, a call with a different or missing path returned `Ok`. The reviewer confirmed the dynamic-load contract, the v1.28.3 pins against Microsoft's release API, and GLIBC_2.27 with `readelf -V`. Fix round 1 (tests first at `c2e1d71`): a three-state record with a cached failure, and a canonical-path check.
- Review 2 on `edaca13`: PASS. Both regressions pass through the public entry in fresh processes. Canonical aliases, a junction, and concurrent loading pass on Windows. A genuine older runtime build is not tested.
- Gates on the task branch: exit 0 at `f87d788` (cargo 148, pytest 19) and `a5d31f9` (cargo 151, pytest 19). Hosted CI runs `38118459700` on `7f5450c` and `38122075416` on `edaca13`: both pass on `ubuntu-22.04` and `windows-latest`, with the parity test on both. Hosted pytest is 12 passed and 3 skipped, because the export dependency group is off in CI.
- Integration: cherry-picked onto task 16's integration branch. `crates/melomae-core/Cargo.toml` conflicted (the dynamic `ort` line next to `realfft`). `Cargo.lock` merged cleanly; every package version in it appears in a parent lock. The orchestrator ran the gate on the merged tree: `pnpm verify` exit 0, board 57, tasks unittest 9, Vitest 1, cargo test 179, pytest 19.
- Backlog filed: B-027 (verify a cached ONNX Runtime library).

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

- [x] Hosted CI passes on `ubuntu-22.04` and `windows-latest` with the parity test running on both.
- [x] `Outcome` records the ONNX Runtime release, its SHA-256 values, and its glibc requirement.
- [x] The new tests failed first for the expected reason.
- [x] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core inference
pnpm verify
```
