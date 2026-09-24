# 28 — Build and verify the Linux packages

**Status:** TODO
**Phase:** 5 — Packaging and release
**Depends on:** 25, 26
**SRS:** §2, §3
**Stack:** §A, §J
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Produce Linux packages and run them as far as this Windows machine allows.

## Evidence

- WSL2 Ubuntu is installed on the dev machine. It is stopped. WSLg can show Linux GUI apps.
- Tauri 2 on Linux needs WebKitGTK 4.1.

## Scope

1. Build an AppImage and a .deb on Ubuntu 22.04 (glibc 2.35 baseline) against WebKitGTK 4.1. The CI workflow runs this build once bootstrap item 2 exists.
2. Build locally in WSL2 Ubuntu. Installing the Tauri prerequisites in WSL writes system packages. Ask the owner first.
3. Smoke-run the AppImage under WSLg: complete a CPU restore and an A/B playback.

## Out of scope

- A real desktop Linux on physical hardware. This machine cannot test it.

## Test-first plan

Add a build check that fails when `apollo.onnx` or the ONNX Runtime library is missing from the AppImage. Run it before packaging. It fails.

## Acceptance criteria

- [ ] The AppImage and the .deb build.
- [ ] The WSLg smoke run passes. `Outcome` records the steps.
- [ ] `Outcome` names what stays unverified: X11 and Wayland on real hardware, and GPU on a native Linux install.
- [ ] The new check failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

Run in WSL2 Ubuntu:

```bash
pnpm install --frozen-lockfile
pnpm tauri build
```

Record the smoke run in `Outcome`.
