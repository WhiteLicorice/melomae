# 27 — Build and verify the Windows installer

**Status:** TODO
**Phase:** 5 — Packaging and release
**Depends on:** 25, 26
**SRS:** §2, §3
**Stack:** §J
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Produce a Windows installer that a non-technical user can run, and prove it works on a machine state that lacks the dev tools.

## Evidence

- The dev machine runs Windows 11 Home. Home has no Windows Sandbox and no Hyper-V.
- WebView2 153.0 is installed on the dev machine.
- Bootstrap items 3 (the app identifier) and 4 (code signing) apply.

## Scope

1. Build a per-user NSIS installer that needs no admin rights.
2. Use the WebView2 download bootstrapper.
3. Bundle the ONNX Runtime libraries, the chosen GPU provider files (task 08), and `apollo.onnx` as resources.
4. Build `apollo.onnx` in the release path from the pinned checkpoint with `tools/export`. Verify the checkpoint SHA-256.
5. Keep the installer at 150 MB or less.
6. Apply the signing decision from bootstrap item 4.

## Out of scope

- Auto-update (backlog B-006)

## Test-first plan

Add a build check that fails when the installer exceeds 150 MB or when `apollo.onnx` is missing from the bundle. Run it before the resources exist. It fails.

## Acceptance criteria

- [ ] Under a fresh local standard user account, with no conda, Python, or ffmpeg on PATH, the app installs and completes a CPU restore. `Outcome` records the steps and times.
- [ ] `Outcome` records the installer size.
- [ ] A clean VM run is optional and human-owned. Record whether it happened.
- [ ] The new check failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
pnpm tauri build
pnpm verify
```

Record the manual journey in `Outcome`.
