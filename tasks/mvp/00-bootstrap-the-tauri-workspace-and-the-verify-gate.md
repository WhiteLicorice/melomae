# 00 — Bootstrap the Tauri workspace and the verify gate

**Status:** DONE
**Phase:** 0 — Foundation and go/no-go spikes
**Depends on:** —
**SRS:** §2, §11
**Stack:** §A, §B, §I
**Assigned to:** Agent
**Started:** 2026-09-24 — No repository existed before this task. Baseline gate: `python tasks/validate_board.py` prints "Task board valid: 41 records."; `python -m unittest discover -s tasks -p "test_*.py"` prints 9 passed. `pnpm --version` failed with MODULE_NOT_FOUND. `git init` run in this task; no commit. Safe files added: `LICENSE`, `.gitignore`, `pytest.ini`, `tools/`, `.github/workflows/verify.yml`.
**Outcome:** DONE 2026-09-24. `pnpm verify` passes every Stack §I step. Counts: board 41 records; tasks unittest 9 passed; frontend typecheck and lint clean; Vitest 1 passed; Vite build ok; `cargo fmt --check` clean; `cargo clippy --workspace -- -D warnings` clean; `cargo test --workspace` 1 passed (the `melomae_core_has_no_tauri_dependency` guard); `uv run --project tools pytest` 1 passed. Red-first: the Vitest test failed on the missing "Melomae" heading, and `cargo test --workspace` failed with "could not find `Cargo.toml`". `pnpm tauri build --debug` produced `target/debug/melomae.exe` plus the MSI and NSIS bundles. `pnpm tauri dev` opened a window titled "Melomae" (read from `MainWindowTitle`, then the process tree was killed; port 1420 is free). pnpm repair: installed 12.6.0 standalone to `%LOCALAPPDATA%\pnpm`, then ran `corepack disable pnpm`. Versions installed: pnpm 12.6.0, Node 24.9.0, @tauri-apps/cli 2.11.5, @tauri-apps/api 2.11.1, tauri 2.11.6, react 19.3.0, vite 8.3.0, typescript 6.0.3, vitest 5.0.1, eslint 10.11.0, typescript-eslint 8.70.1, cargo_metadata 0.23.1, rustc and cargo 1.95.0, uv 0.12.11, pytest 9.1.1. Not verified: a true clean-clone install, because no commit exists yet. No commit was made, per the task. Post-DONE change on 2026-09-24: the owner chose the minimal Git style. The SPDX header policy from scope item 7 and all per-file headers were removed. The license statement and the copyright notice now live only in `LICENSE` and `README.md`.

## Goal

Create a bootable Tauri 2 + React + Vite + TypeScript app, a Cargo workspace, and one `pnpm verify` gate.

## Evidence

- `pnpm` on PATH is broken. `which -a pnpm` resolves to `C:\nvm4w\nodejs\pnpm`. This is the corepack 0.34.0 shim from Node 24.9.0.
- The shim loads `%LOCALAPPDATA%\node\corepack\v1\pnpm\12.6.0\bin\pnpm.cjs` and fails with "Cannot find module". pnpm 12.6.0 ships `bin\pnpm.mjs`.
- `node %LOCALAPPDATA%\node\corepack\v1\pnpm\12.6.0\bin\pnpm.mjs --version` prints `12.6.0`.
- `%LOCALAPPDATA%\pnpm` exists with an empty `bin\` and a `store\`.
- The Tauri CLI is not installed. Rust 1.95.0 (MSVC), Visual Studio 2022 VC tools, WebView2 153.0, and uv 0.12.11 are present (Stack §K).

## Scope

1. **Repair pnpm on PATH. Ask the owner first.** This writes the global PATH and the shared Node install directory.
   - Measured on 2026-09-24: the effective PATH is machine-first. `C:\nvm4w\nodejs` sits at machine index 27. Every user PATH entry follows it. So adding `%LOCALAPPDATA%\pnpm` to the user PATH cannot place it ahead of `C:\nvm4w\nodejs`. The original instruction is insufficient.
   - `C:\nvm4w\nodejs` is a symbolic link to `C:\Users\Ren\AppData\Local\nvm\v24.9.0`. It is writable without elevation.
   - Recommended: install pnpm 12.6.0 standalone with `PNPM_HOME=%LOCALAPPDATA%\pnpm` through the official `get.pnpm.io/install.ps1`. Then run `corepack disable pnpm` to remove the incompatible corepack 0.34.0 shim, so it cannot shadow the new install.
   - Alternative: run `npm i -g pnpm@12.6.0`. This is the method pnpm's Windows documentation recommends. It writes into the same Node directory and needs no PATH edit.
   - Alternative: upgrade corepack. Unverified. Corepack 0.34.0 resolves `bin\pnpm.cjs`. pnpm 12 ships a native binary (`bin` maps to `pnpm`; the corepack cache holds `pnpm.mjs`).
   - Rollback: `corepack enable pnpm` restores the shim. Delete the `PNPM_HOME` user environment variable and the `%PNPM_HOME%\bin` user PATH entry. Delete `%LOCALAPPDATA%\pnpm`.
2. Run `git init`. Do not commit without the owner's yes.
3. Pin pnpm in `packageManager`. Use the version and hash from corepack's `lastKnownGood.json` (`pnpm@12.6.0+sha512…`). Confirm that 12.6.0 is still current.
4. Scaffold Tauri 2 with React, Vite, and TypeScript in strict mode. Add `@tauri-apps/cli` as a dev dependency.
5. Create the Cargo workspace:
   - `crates/melomae-core`: no `tauri*` dependency.
   - `crates/melomae-cli`: a binary that prints its version.
   - `src-tauri`: the app.
6. Create the uv project `tools/` with pytest.
7. Add `LICENSE` with the GPL-3.0-or-later text. Add a short SPDX header policy to the README.
8. Add a `.gitignore` that ignores `node_modules`, `target`, model artifacts (`*.onnx`, `*.bin`), and the uv environment.
9. Add `pnpm verify`. It runs the checks in Stack §I, in that order.
10. Add a GitHub Actions workflow on `windows-latest` and `ubuntu-22.04`. It runs `pnpm verify`. It cannot run until bootstrap item 2 exists.
11. Write a root `README.md` with the dev commands.

Confirm every version from its primary registry at install time. Record the versions in `Outcome`.

## Out of scope

- Product screens beyond a boot shell
- The model and the audio code

## Test-first plan

1. Write a Vitest smoke test that renders the shell and finds the title "Melomae". Run it. It fails because the shell does not exist.
2. Write a Rust test in `melomae-core`. It runs `cargo metadata` and fails if any package in the core's dependency tree starts with `tauri`. Run it. It fails because the workspace does not exist.

## Acceptance criteria

- [x] `pnpm --version` prints the pinned version in a new PowerShell session and a new Git Bash session. (The harness shell has a stale PATH, so the fresh-session PATH was rebuilt from the machine and user registry values.)
- [x] `pnpm install --frozen-lockfile` passes on a clean clone. (Run in place; the lockfile is up to date. A true clean clone needs the first commit.)
- [x] `pnpm tauri dev` opens a window with the title "Melomae".
- [x] `melomae-core` has no Tauri dependency, and its test proves it.
- [x] `pnpm verify` runs every step in Stack §I and passes.
- [x] The new tests failed first for the expected reason.

## Verify

```powershell
pnpm --version
pnpm install --frozen-lockfile
pnpm verify
pnpm tauri build --debug
python tasks/validate_board.py
```
