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

## Owner decisions (2026-10-11)

The owner made all decisions below on 2026-10-11. This is not legal advice. Each decision names its sources. The executor checks each cited `file:line` and each tool behavior against the pinned version before it acts.

### Part 1: text for `docs/melomae-srs.md` §11

Add these items after §11.6.

7. **The model is a separate work.** `apollo.onnx` ships beside the app under CC BY-SA 4.0. The app stays GPL-3.0-or-later.
   - The app loads the model at runtime from a path that the caller gives (`crates/melomae-core/src/inference/onnx.rs:3`). The installer adds the model as a resource (Stack §J). The shipped app contains no Apollo source.
   - GPLv3 §5 calls separate works on one medium an "aggregate" (`LICENSE:243-251`). The GPL does not apply to the model.
   - The model keeps its own license, so Melomae uses no BY-SA Compatible License. CC BY-SA 4.0 §3(b)(1) and §1(c) admit a later GPL version only if Creative Commons lists it. Because no compatible license is used, this limit does not apply.
   - CC BY-SA 4.0 §2(a)(4) can make the ONNX conversion a technical modification and not Adapted Material. The decision holds under both readings.
   - The installer and the app terms must not restrict the copying of `apollo.onnx`. They must not encrypt the model (CC BY-SA 4.0 §2(a)(5)(c) and §3(b)(3)).
   - Rejected: GPL-3.0-only for the release that bundles the model. It gives up "or later" for no gain. Rejected: a GPLv3 §14 proxy. It adds a decision-maker and assumes that app and model are one work.
   - Sources: CC BY-SA 4.0 legal code. GPLv3 (`LICENSE`). https://wiki.creativecommons.org/wiki/ShareAlike_compatibility:_GPLv3

8. **The LLVM exception.** `deny.toml` admits `Apache-2.0 WITH LLVM-exception` through a per-crate exception for `target-lexicon` only. The global `allow` list does not contain it.
   - `target-lexicon 0.12.16` is the only crate under this license. It is a build dependency only: `cfg-expr` → `system-deps`, under `[build-dependencies]` of the Linux GTK `-sys` crates (`cargo tree -i target-lexicon -e normal,build --target all`). It does not ship in the installer.
   - The FSF lists Apache-2.0 as compatible with GPLv3. The LLVM exception only adds permissions.
   - A new crate under this license fails the gate until the owner reviews it.
   - Rejected: a global allow, which admits future crates without review. Rejected: `[licenses] include-build = false`, which removes every build dependency from the check. Rejected: `[graph] exclude`, which also hides the crate from the advisory, ban, and source checks.

9. **The training-audio rights.** The release ships the model and does not wait.
   - Apollo was trained on MUSDB18-HQ and MoisesDB (https://github.com/JusperLee/Apollo). MUSDB18 tracks "can only be used for academic purposes" (https://github.com/sigsep/website/blob/master/content/datasets/musdb.md). MoisesDB is CC BY-NC-SA 4.0 (https://github.com/moises-ai/moises-db/blob/main/LICENSE).
   - The owner relies on the CC BY-SA 4.0 grant for the weights. Melomae follows CC BY-SA 4.0 and does not treat the dataset terms as binding.
   - CC BY-SA 4.0 §1(i) limits the grant to rights that the Licensor has authority to license. §5(a) gives no warranty of non-infringement.

10. **The hosted backend.** The backend (B-012) can run the model on the terms in §11.9.
    - CC BY-SA 4.0 §3 applies only when You Share the material. §1(k) defines Share as providing material to the public. A model that runs on the server only is not Shared.
    - Whether restored audio is Adapted Material (§1(a)) is not settled. The backend credits Apollo and obeys §11.4.
    - If the backend sends the model to clients, §3(a) and §3(b) apply. The backend then must not encrypt the model.
    - CC BY-SA 4.0 §2(b)(2) grants no patent or trademark rights.

### Part 2: steps for the executor

Do the steps in this order. Run `pnpm verify` after each step.

1. Add §11.7 to §11.10 from Part 1 to `docs/melomae-srs.md`.
2. Change `deny.toml`:
   - Remove `"Apache-2.0 WITH LLVM-exception"` from `[licenses] allow`.
   - Run `cargo deny check licenses`. It must fail on `target-lexicon`. Record that output.
   - Add `exceptions = [{ allow = ["Apache-2.0 WITH LLVM-exception"], crate = "target-lexicon" }]` under `[licenses]`. Add a comment that says the crate is build-only, through `system-deps`.
   - Run `cargo deny check licenses` again. It must pass.
   - Caution: the cargo-deny docs on `main` use the key `crate`. Older versions use `name`. Check the key against the cargo-deny version that the gate pins.
3. Check `models/NOTICE.md` against CC BY-SA 4.0 §3(a)(1). It must contain:
   - the creators (Kai Li and Yi Luo)
   - the upstream copyright notice, if upstream supplies one
   - a reference to CC BY-SA 4.0
   - a reference to the disclaimer of warranties in §5
   - a link to https://github.com/JusperLee/Apollo
   - the statement of changes: converted to ONNX, STFT and iSTFT expressed as real arithmetic
   - the license name and https://creativecommons.org/licenses/by-sa/4.0/legalcode

   `models/README.md` has no reference to the disclaimer today. Add one to the README too.
4. Add a license section to `README.md` that gives the license for each path:
   - GPL-3.0-or-later: the Melomae code
   - CC BY-SA 4.0: `models/apollo.onnx`, `tools/export/vendor/`, `fixtures/ori_wav.wav`, `tools/parity/input_wav.wav`

   The top of `LICENSE` says all of Melomae is GPL-3.0-or-later. Make it point to the README section.
5. Fix the license fields:
   - `src-tauri/Cargo.toml` is the crate that ships the app, and it has no `license` field. Add `license.workspace = true`. Its `description = "A Tauri App"` and `authors = ["you"]` are still Tauri template values. Report them to the owner. Do not change them without approval.
   - `package.json` and `src-tauri/tauri.conf.json` have no license field. Add none unless the gate needs one.
6. Add one line to `tasks/backlog/B-012-offer-optional-hosted-restore.md` under "Expected behavior": "SRS §11.10 records the license terms for hosted use of the model."
7. Run `pnpm verify`. Record the result in the Task 35 `Outcome`.

### Part 3: open item for the owner

Do not decide this item. Report it to the owner.

- `tools/export/real_dsp.py` defines `RealApollo`. It reuses the Apollo submodules, and its `forward` may follow `Apollo.forward` in `tools/export/vendor/look2hear/models/apollo.py`. Nobody has compared the two line by line. If `real_dsp.py` copies upstream logic, it is adapted CC BY-SA 4.0 material. It is then not plain GPL code, and step 4 must list it.

### Evidence limits

- The CC wiki page in §11.7 was blocked by the network policy and was not read directly. The license text of CC BY-SA 4.0 came from the owner.
- The `include-build`, `exceptions`, and `[graph] exclude` behavior comes from the cargo-deny docs on `main`, not from the pinned version.

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
