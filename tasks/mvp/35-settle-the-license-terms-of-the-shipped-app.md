# 35 — Settle the license terms of the shipped app

**Status:** TODO
**Phase:** 5 — Packaging and release
**Depends on:** 26
**SRS:** §11
**Stack:** §C, §J
**Assigned to:** Owner
**Started:** —
**Outcome:** —

## Goal

Decide, and record in the repository, the license terms under which Melomae ships with the Apollo model and its dependencies, so that no installer ships on an open license question.

## Evidence

- Melomae code is GPL-3.0-or-later (SRS §11.1, `Cargo.toml` `license`). The Apollo code and weights are CC BY-SA 4.0 (SRS §11.2). The installer bundles `models/apollo.onnx` (Stack §J).
- Creative Commons lists GPLv3 as a one-way compatible license for CC BY-SA 4.0 (https://creativecommons.org/compatible-licenses/). Its analysis discusses the choice of later GPL versions (https://wiki.creativecommons.org/wiki/ShareAlike_compatibility:_GPLv3). GPLv3 §14 lets a project name a proxy that decides whether a later GPL version applies. Whether bundling the model with the app makes one adapted work is not settled. The task 26 review (2026-10-11) did not infer that it does.
- The task 26 allowlist (`deny.toml`) admits `Apache-2.0 WITH LLVM-exception` for `target-lexicon 0.12.16`. The FSF lists Apache-2.0 as GPLv3-compatible. The FSF list has no entry for the `WITH LLVM-exception` form. The task 26 executor inferred compatibility because the exception only adds permissions.
- The rights in the audio that trained the Apollo model are not confirmed in this repository. `fixtures/README.md` records the same gap for the triage fixture. The upstream model card names the training datasets but does not establish the recording permissions.
- The task 26 executor and reviewer raised these three questions on 2026-10-11. The author asked for them to be an MVP task, not a note in a task outcome or a backlog item.

## Scope

1. **The model and "or later".** Decide one of these, and record the reason:
   - Melomae ships under GPL-3.0-or-later and names a proxy under GPLv3 §14 for the combined work.
   - The release that bundles the model ships under GPL-3.0-only.
   - The model is a separate work beside the app, with its own CC BY-SA 4.0 terms, and the app stays GPL-3.0-or-later.
2. **The LLVM exception.** Confirm the `Apache-2.0 WITH LLVM-exception` allowlist entry, or replace it with a per-crate exception in `deny.toml`.
3. **The training-audio rights.** Record what is known about the rights in the Apollo training data, and decide whether the release ships the model with that gap stated, or waits.
4. Record each decision in `docs/melomae-srs.md` §11, and update `deny.toml`, `models/NOTICE.md`, the license fields, and the README where a decision changes them.

## Out of scope

- Legal advice. The author may consult a lawyer. The task records the decision and its sources.
- The notices file itself (task 26).

## Acceptance criteria

- [ ] Each of the three questions has a recorded decision with its reason and sources in SRS §11.
- [ ] `deny.toml`, `models/NOTICE.md`, and the license fields agree with the decisions.
- [ ] `pnpm verify` passes.

## Verify

```powershell
pnpm verify
```
