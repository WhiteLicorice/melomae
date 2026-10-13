# 26 — Publish third-party notices and the model attribution

**Status:** DONE
**Phase:** 5 — Packaging and release
**Depends on:** 03, 04, 06
**SRS:** §11
**Stack:** §C, §D, §I
**Assigned to:** Agent
**Started:** 3690b93, clean tree. Baseline gate at 3690b93: board 61 records, tasks unittest 9 passed, Vitest 1 passed, cargo test 179 passed, pytest 19 passed.
**Outcome:** DONE. Added the cargo-deny license gate and the third-party notices. `tools/cargo_deny.py` installs one pinned prebuilt cargo-deny release (0.20.2) into `.cache/cargo-deny` with a checked SHA-256, then runs it. `pnpm verify` runs `cargo deny check licenses`. `deny.toml` allows the licenses the tree uses that are compatible with GPL-3.0-only, and cites the FSF license list for each. `Apache-2.0 WITH LLVM-exception` (target-lexicon 0.12.16) has no direct FSF entry; it is a per-crate exception in `deny.toml`, allowed because the exception only adds permissions, so the Apache-2.0 GPLv3 compatibility holds. `LGPL-2.1-or-later` is not allowed: it appears only in `r-efi`'s choice, and Melomae picks MIT or Apache-2.0. `src-tauri` did not declare a license, so cargo-deny failed the app crate; it now sets `license.workspace = true`. `tools/generate_notices.py` builds `THIRD_PARTY_NOTICES.md` from the non-dev Cargo graph for all targets and the production pnpm tree, plus the two bundled components that no manifest lists (ONNX Runtime 1.28.3, MIT, and `apollo.onnx`, CC-BY-SA-4.0). It keeps each component's own license, notice, and copyright texts, including a notice kept beside the code it covers (for example `src/unicode_tables/LICENSE-UNICODE`), read from the source the component ships, and groups only byte-identical texts (each group lists every component it covers). Apache `NOTICE` files and `COPYRIGHT` files are kept in full, and a component that ships any such document never gets the standard SPDX fallback. The generated declaration states GPL version 3 only, matching `LICENSE`. It reads the bundled ONNX Runtime `LICENSE` and `ThirdPartyNotices.txt` verbatim from the pinned release and fails when one is missing, includes the vendored libopus and the SQLite public-domain notice, and falls back to the committed standard SPDX texts for a component that ships none (`tools/spdx_texts/`). `pnpm verify` runs `python tools/generate_notices.py --check`, which fails when the file is stale, so tasks 05 and 15 must regenerate it. `models/NOTICE.md` meets CC BY-SA 4.0 section 3(a)(1) (the creators Kai Li and Yi Luo, the absence of a supplied upstream copyright notice, CC BY-SA 4.0 and its `legalcode` link, the section 5 disclaimer of warranties, the source, and the ONNX and real-arithmetic changes). Red first: `cargo deny check licenses` failed (exit 4) on `mp3lame-encoder` 0.2.5 (LGPL-3.0), and the notices check failed (exit 1) on a stale file. The gate `pnpm verify` exits 0 at the fix-round-2 head: board 69 records, tasks unittest 9 passed, Vitest 1 passed, cargo test 193 passed and 3 ignored, pytest 45 passed (19 baseline plus 26 new), cargo deny licenses ok, and the notices file current. The three license questions this task raised are settled by task 35 (SRS §11.7 to §11.11). `src-tauri/Cargo.toml` carries the author's 2026-10-13 ruling for the app description and author. `src-tauri/tauri.conf.json` has no bundle resources list, so tasks 27 and 28 must add `THIRD_PARTY_NOTICES.md` and `models/NOTICE.md` to it; both task files now name that step.

## Goal

Ship every license notice that Melomae's bundled components require, and keep the gate from admitting a dependency with an unapproved license.

## Evidence

- The Apollo code and weights are CC BY-SA 4.0 (the local `LICENSE` and the Hugging Face card). CC BY-SA 4.0 is one-way compatible with GPLv3.
- Symphonia is MPL-2.0. libopus is BSD-3-Clause. rubato is MIT. ONNX Runtime is MIT.

## Scope

1. Add `cargo-deny` with a license allowlist to the gate. Every allowed license must be compatible with GPL-3.0-only distribution.
2. Generate `THIRD_PARTY_NOTICES` from the Cargo and pnpm dependency trees. Bundle it with the app.
3. Add a model notice: the creators, CC BY-SA 4.0 with a link, the source URL, and the modification statement ("converted to ONNX, STFT re-expressed as real arithmetic").
4. Ship `apollo.onnx` under CC BY-SA 4.0.

This is not legal advice. Record any open license question for the owner in `Outcome`.

## Out of scope

- The About screen (task 25)

## Test-first plan

Add a test dependency with a license outside the allowlist in a temporary branch of the manifest. `cargo deny check licenses` fails. Remove the dependency. The check passes.

## Acceptance criteria

- [x] `cargo deny check licenses` is part of `pnpm verify` and passes.
- [x] The notices file lists every bundled component.
- [x] The new check failed first for the expected reason.
- [x] `pnpm verify` passes.

## Verify

```powershell
cargo deny check licenses
pnpm verify
```
