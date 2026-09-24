# 26 — Publish third-party notices and the model attribution

**Status:** TODO
**Phase:** 5 — Packaging and release
**Depends on:** 03, 04, 06, 11
**SRS:** §11
**Stack:** §C, §D, §E, §I
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Ship every license notice that Melomae's bundled components require, and keep the gate from admitting a dependency with an unapproved license.

## Evidence

- The Apollo code and weights are CC BY-SA 4.0 (the local `LICENSE` and the Hugging Face card). CC BY-SA 4.0 is one-way compatible with GPLv3.
- Symphonia is MPL-2.0. libopus is BSD-3-Clause. rubato is MIT. ONNX Runtime is MIT.
- yt-dlp's PyInstaller binaries include GPLv3+ code. Deno is MIT. Melomae downloads both at runtime and does not bundle them.

## Scope

1. Add `cargo-deny` with a license allowlist to the gate. Every allowed license must be compatible with GPL-3.0-or-later distribution.
2. Generate `THIRD_PARTY_NOTICES` from the Cargo and pnpm dependency trees. Bundle it with the app.
3. Add a model notice: the creators, CC BY-SA 4.0 with a link, the source URL, and the modification statement ("converted to ONNX, STFT re-expressed as real arithmetic").
4. Ship `apollo.onnx` under CC BY-SA 4.0.
5. State in the notices that yt-dlp and Deno download at runtime, with their licenses.

This is not legal advice. Record any open license question for the owner in `Outcome`.

## Out of scope

- The About screen (task 25)

## Test-first plan

Add a test dependency with a license outside the allowlist in a temporary branch of the manifest. `cargo deny check licenses` fails. Remove the dependency. The check passes.

## Acceptance criteria

- [ ] `cargo deny check licenses` is part of `pnpm verify` and passes.
- [ ] The notices file lists every bundled component.
- [ ] The new check failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo deny check licenses
pnpm verify
```
