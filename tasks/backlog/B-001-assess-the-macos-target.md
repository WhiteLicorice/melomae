# B-001 — Assess the macOS target

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 04

## Impact

The briefer names macOS as a desirable third target (`docs/source/briefer.md`, "Stretch scope"). The MVP ships for Windows and Linux only.

## Evidence

- The Apollo clone records an Apple M3 test with torch 2.11: MPS took 5.78 s and CPU took 316.46 s for a 6 s file (`C:\Lab\Apollo\MACOS_ARM64.md:79-82`).
- That CPU figure does not match the x86 dev machine, where torch CPU runs at RTF 2.70 (Stack §K). The cause is unknown.
- torch no longer ships Intel macOS builds. ONNX Runtime still ships an x86_64 macOS build, so the ONNX path keeps Intel Macs possible. Confirm this live before the assessment.

## Why this does not block MVP

SRS §2 limits the MVP to Windows and Linux. No MVP journey needs macOS.

## Expected behavior

The assessment measures ONNX Runtime CPU and the CoreML execution provider on Apple Silicon, for speed and parity. It checks Intel Mac CPU speed. It records the cost and the steps of notarization. It ends with a recommendation.

## Promotion condition

Promote this task when the owner wants a macOS release and has Apple hardware and an Apple Developer account.

## Acceptance criteria

- [ ] CPU and CoreML RTF and parity numbers on Apple Silicon.
- [ ] An Intel Mac CPU number, or a stated reason why none exists.
- [ ] The notarization cost and steps, with sources and dates.
