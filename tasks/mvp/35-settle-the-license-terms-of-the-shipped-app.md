# 35 — Settle the license terms of the shipped app

**Status:** DONE
**Phase:** 5 — Packaging and release
**Depends on:** 26
**SRS:** §11
**Stack:** §C, §J
**Assigned to:** Owner
**Started:** 2fcf1ba, clean worktree `SCRATCH\wt-35` on branch `task/35`. Baseline gate at master `53dd039` (task 34 merged): board 65 records, tasks unittest 9, Vitest 1, cargo test 181, pytest 19.
**Outcome:** DONE. Part 1 local check (`SCRATCH/evidence-35/part1.txt`). The clone `C:\Lab\Apollo` has origin `WhiteLicorice/Apollo` (a fork) and remote upstream `JusperLee/Apollo`. Its `inference.py` is byte-identical to upstream `JusperLee/Apollo@e84bcac`. `git blame -L 86,221` names Djshinshin and Zakaria Zbitou for the ported lines and the owner for none. Upstream `e84bcac` contains the four chunked functions. Result A holds, so Parts 2 and 3 apply as written. SRS §11 now records GPL-3.0-only (§11.1) and §11.7 through §11.11. `crates/melomae-core/src/inference/mod.rs` and `tools/export/real_dsp.py` carry the CC BY-SA 4.0 §3(a) attribution. `Cargo.toml`, `src-tauri/Cargo.toml` (`license.workspace = true`), `LICENSE`, `README.md`, task 25, task 26 scope, B-012, and the pivot record are relicensed to GPL-3.0-only. The Tauri template `description = "A Tauri App"` and `authors = ["you"]` in `src-tauri/Cargo.toml` are unchanged. Gate `pnpm verify` exit 0 (`SCRATCH/evidence-35/verify.txt`, ending `EXIT=0`): board 66 records valid, tasks unittest 9 passed, Vitest 1 passed, cargo test 181 passed, pytest 19 passed. Cure deadline for §11.11 is 2026-11-10. Task 26 carries the `deny.toml` per-crate LLVM exception and the `models/NOTICE.md` checklist, because both files exist only on the unmerged `task/26` branch.

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

## Amendment 1 (2026-10-11)

Date: 2026-10-11. This is not legal advice.

This amendment changes the "Owner decisions (2026-10-11)" section above. Where the two disagree, this amendment wins. It also closes the open item in Part 3 above (the license of `tools/export/real_dsp.py`).

**Orchestrator notes for the executor.**

- `C:\Lab\Apollo` is read-only (author rule). Part 1 step 2 (`git -C C:\Lab\Apollo fetch origin`) writes objects and refs into that clone's `.git`, so do not run it. Clone `https://github.com/JusperLee/Apollo` into the scratch directory. Run steps 3 to 6 against the local clone with read-only commands, and against the scratch clone's `origin/main`. Set `PYTHONDONTWRITEBYTECODE=1`.
- Part 1 step 9 is answered. On 2026-10-11 the owner said: "No, neither." The owner is not Djshinshin or zbitouzakaria and wrote none of the ported lines. Do not ask again.

### What changes

1. Melomae code is now **GPL-3.0-only**. It was GPL-3.0-or-later. There is no GPLv3 §14 proxy.
2. The decisions' §11.7 bullet "The shipped app contains no Apollo source" is **false**. `crates/melomae-core/src/inference/mod.rs` ships in the app and translates upstream Apollo code. Part 2 replaces §11.7.
3. New SRS §11.11 covers the ported Apollo code.
4. `tools/export/real_dsp.py` is GPL-3.0-only with the Apollo attribution. Decisions Part 3 is closed.
5. A local check of `C:\Lab\Apollo\inference.py` comes first (Part 1).
6. A cure date applies: **2026-11-10** (see §11.11).

These parts stay as they were: decisions §11.8, §11.9, §11.10, and decisions steps 2 (`deny.toml`), 3 (`models/NOTICE.md`), and 6 (B-012).

### Part 1: local check (do this first)

The session that wrote this ran in a remote container. It could not read `C:\Lab\Apollo`. These facts came from the repository and from GitHub on 2026-10-11:

- `crates/melomae-core/src/inference/mod.rs:3-4` says it ports `C:\Lab\Apollo\inference.py:86-221`.
- Lines 191, 241, 253, and 284 name the ported functions: `resolve_chunking`, `chunk_starts`, `crossfade_weights`, and `run_model`.
- Upstream `JusperLee/Apollo` `main` has all four functions in `inference.py`.
- Upstream history of `inference.py` on `main`:

| Commit | Author | Date | Message |
|---|---|---|---|
| 10b4427 | zbitouzakaria | 2026-08-23 | Clamp the padded chunk start and derive the crop offset from it |
| 99b3187 | zbitouzakaria | 2026-08-21 | Pad chunk inference and discard the model's degraded edges |
| 32f7579 | Djshinshin | 2026-08-14 | Add batched chunk inference |
| 0a91c5f | Djshinshin | 2026-08-14 | Add chunked inference for long audio |
| 46c09a1 | Djshinshin | 2026-08-13 | Address inference review feedback |
| e121224 | Djshinshin | 2026-08-13 | Add device-agnostic inference support |
| fb56d56 | JusperLee | 2024-09-18 | Update inference.py |
| d72b12c | JusperLee | 2024-09-06 | First Commit |

- Stack §K says the clone's only local change is `32a2d35`, which removes a `print`. The Stack pins upstream commit `e84bcac`.

Run these in PowerShell. Record each output in the Task 35 `Outcome`.

1. `git -C C:\Lab\Apollo remote -v`. Confirm that `origin` is `JusperLee/Apollo`.
2. `git -C C:\Lab\Apollo fetch origin`. This only reads from the remote. (See the orchestrator notes above: use a scratch clone instead.)
3. `git -C C:\Lab\Apollo log --format="%h %an %ad %s" --date=short -- inference.py`. Compare the result with the table above.
4. `git -C C:\Lab\Apollo diff origin/main -- inference.py`. Record any local change.
5. `git -C C:\Lab\Apollo blame -L 86,221 inference.py`. Record the authors of the ported lines.
6. `git -C C:\Lab\Apollo rev-parse HEAD`. Then run `git -C C:\Lab\Apollo show e84bcac:inference.py | Select-String "def resolve_chunking"`. Record the clone's commit, and record whether `e84bcac` contains the chunked functions.
7. Compare each of the four functions with its Rust counterpart in `mod.rs`. Confirm that the Rust code translates them function by function.
8. Compare the inference tests in `crates/melomae-core/tests` with upstream `tests/test_inference.py`. Record whether the tests translate code or only reuse ideas.
9. Ask the owner whether they are Djshinshin or zbitouzakaria, or wrote any of the ported lines. (Answered: see the orchestrator notes above.)

Then act on the result:

- **Result A (expected):** others wrote the ported lines upstream. Apply Parts 2 and 3 as written.
- **Result B:** the owner wrote every ported line. The owner holds that copyright. GPL-3.0-only stands anyway. Change §11.11 to say "a port of the owner's own upstream contribution". Keep the attribution, because it costs nothing.
- **Result C:** the local file is not upstream code, or it differs from upstream in the ported lines. Stop. Report to the owner before you change any file.
- If step 8 shows that tests translate upstream code, give those test files the same attribution header as in Part 3 step 4.

### Part 2: SRS §11 text

Replace §11.1:

1. **Melomae code is licensed GPL-3.0-only.** The owner decided this on 2026-10-11. It replaces GPL-3.0-or-later.
   - The app contains code translated from CC BY-SA 4.0 sources (§11.11). CC BY-SA 4.0 §3(b)(1) and §1(c) accept GPLv3 as the license for that code. They accept a later GPL version only if Creative Commons lists it.
   - One license for all Melomae code keeps the project simple.
   - Git history shows the owner as the only human author. The owner can relicense alone today.
   - Accepted cost: the code has no automatic path to a later GPL version. A later relicense needs the consent of every copyright holder at that time.
   - Rejected: GPL-3.0-or-later with a §14 proxy, because it adds a decision-maker. Rejected: a mix of GPL-3.0-only and GPL-3.0-or-later files, because it is harder to keep correct.

Replace the decisions' §11.7:

7. **The model is a separate work.** `apollo.onnx` ships beside the app under CC BY-SA 4.0. The app is GPL-3.0-only (§11.1).
   - The app loads the model at runtime from a path that the caller gives (`crates/melomae-core/src/inference/onnx.rs:3`). The installer adds the model as a resource (Stack §J). The model is not compiled into the app.
   - GPLv3 §5 calls separate works on one medium an "aggregate" (`LICENSE:243-251`). The GPL does not apply to the model.
   - The model keeps its own license. It needs no BY-SA Compatible License.
   - CC BY-SA 4.0 §2(a)(4) can make the ONNX conversion a technical modification and not Adapted Material. The decision holds under both readings.
   - The installer and the app terms must not restrict the copying of `apollo.onnx`. They must not encrypt the model (CC BY-SA 4.0 §2(a)(5)(c) and §3(b)(3)).
   - Sources: CC BY-SA 4.0 legal code. GPLv3 (`LICENSE`).

§11.8, §11.9, and §11.10 stay as written in the decisions.

Add §11.11:

11. **Ported Apollo code.** The owner decided this on 2026-10-11.
    - `crates/melomae-core/src/inference/mod.rs` translates `resolve_chunking`, `chunk_starts`, `crossfade_weights`, and `run_model` from upstream `inference.py` into Rust. Upstream contributors other than the Apollo authors wrote these functions (Part 1 table).
    - `tools/export/real_dsp.py` computes the same mathematics as `Apollo.spec_band_split`, `Apollo.feature_extractor`, and `Apollo.forward` (`tools/export/vendor/look2hear/models/apollo.py:250-299`), in the same order. It copies no upstream text. The owner treats it as adapted, so that no argument is needed.
    - The upstream repository is CC BY-SA 4.0. CC BY-SA 4.0 §1(a) names translation as a form of adaptation.
    - Both files use GPL-3.0-only as the Adapter's License (§3(b)(1)). Both files carry the §3(a) attribution.
    - Before 2026-10-11, both files carried GPL-3.0-or-later and had no attribution. `mod.rs` named only a local path. Under §6(a), a breach can end the rights. Under §6(b)(1), the rights come back if the breach is cured within 30 days of its discovery. The owner discovered it on 2026-10-11. **Cure deadline: 2026-11-10.** This applies only if the files are Adapted Material.
    - Optional: the owner can ask the upstream copyright holders for other terms (§6(c)). For `inference.py`, those holders include Djshinshin and zbitouzakaria, as well as JusperLee.

### Part 3: executor steps

These steps replace decisions Part 2. Do them in this order. Run `pnpm verify` after each step.

1. Do the local check in Part 1. Stop if you get Result C.
2. Apply Part 2 to `docs/melomae-srs.md`. Add §11.8, §11.9, and §11.10 from the decisions.
3. Do decisions step 2 (`deny.toml`) without change. Then change Task 26 scope line 23 from "GPL-3.0-or-later distribution" to "GPL-3.0-only distribution". No allowlist entry changes. Every license that is compatible with GPL-3.0-or-later is also compatible with GPL-3.0-only. This is inferred, not checked license by license.
4. Add the attribution headers.
   - In `mod.rs`, replace the local path at lines 3-4 with the upstream link and the commit that Part 1 recorded. Add this to the module doc comment:
     ```
     //! Portions of this file translate `resolve_chunking`, `chunk_starts`,
     //! `crossfade_weights`, and `run_model` from `inference.py` in Apollo
     //! (https://github.com/JusperLee/Apollo/blob/<commit>/inference.py),
     //! by Kai Li, Yi Luo, and the Apollo contributors, licensed CC BY-SA 4.0
     //! (https://creativecommons.org/licenses/by-sa/4.0/). Changes: translated
     //! from Python to Rust, with batch size one. This file is licensed
     //! GPL-3.0-only under CC BY-SA 4.0 Section 3(b)(1).
     ```
   - In `real_dsp.py`, add this paragraph to the module docstring:
     ```
     The forward pass follows the computation of Apollo.spec_band_split,
     Apollo.feature_extractor, and Apollo.forward in
     look2hear/models/apollo.py (https://github.com/JusperLee/Apollo), by
     Kai Li and Yi Luo, licensed CC BY-SA 4.0
     (https://creativecommons.org/licenses/by-sa/4.0/). Changes: the STFT and
     iSTFT use real arithmetic, and the three methods are merged into one.
     This file is licensed GPL-3.0-only under CC BY-SA 4.0 Section 3(b)(1).
     ```
5. Relicense to GPL-3.0-only:
   - `Cargo.toml:8`: set `license = "GPL-3.0-only"`.
   - `src-tauri/Cargo.toml`: add `license.workspace = true`. Its `description = "A Tauri App"` and `authors = ["you"]` are Tauri template values. Report them to the owner. Do not change them without approval.
   - `LICENSE` lines 4-5: write "Melomae is licensed under the GNU General Public License, version 3 only. Some files in this repository use other licenses. README.md lists them." Keep the full GPL text below.
   - `README.md:64`: use the same wording as `LICENSE`.
   - Task 25 scope (`tasks/mvp/25-add-the-first-run-setup-and-the-attribution-screen.md:25`): change it to GPL-3.0-only.
   - `tasks/backlog/B-012-offer-optional-hosted-restore.md:17`: change "GPL-3.0-or-later" to "GPL-3.0-only".
   - `docs/evidence/2026-10-10-pivot.md:11`: do not rewrite this historical record. Add after decision 5: "Superseded on 2026-10-11 by SRS §11.1. The client is GPL-3.0-only."
   - Then search the repository for "or-later" and "or later". The only allowed hits are the GPL text, tool version requirements, the pivot record, and the Task 35 history.
6. Add a license section to `README.md` that gives the license of each path. This replaces decisions step 4.
   - GPL-3.0-only: the Melomae code. `mod.rs` and `real_dsp.py` carry the Apollo attribution.
   - CC BY-SA 4.0: `models/apollo.onnx`, `tools/export/vendor/`, `fixtures/ori_wav.wav`, and `tools/parity/input_wav.wav`.
7. Do decisions step 3 (`models/NOTICE.md`) and decisions step 6 (B-012 line) without change.
8. Run `pnpm verify`. Record in the Task 35 `Outcome`: the Part 1 output, the result (A, B, or C), the gate result, and the cure date 2026-11-10.

### Evidence limits

- This amendment comes from a session that could not read `C:\Lab\Apollo`. Part 1 closes that gap.
- The upstream history came from the GitHub page for `main` on 2026-10-11. It does not show whether `e84bcac` contains the chunked functions.
- The 30-day reading of §6(b)(1) comes from the license text that the owner supplied. It applies only if the files are Adapted Material.
- The `deny.toml` key name (`crate` or `name`) still needs checking against the cargo-deny version that the gate pins.

## Out of scope

- Legal advice. The author may consult a lawyer. The task records the decision and its sources.
- The notices file itself (task 26).

## Acceptance criteria

- [x] Each of the three questions has a recorded decision with its reason and sources in SRS §11.
- [x] The license fields agree with the decisions. `deny.toml` and `models/NOTICE.md` are task 26's, and both exist only on the unmerged `task/26` branch.
- [x] `pnpm verify` passes.

## Verify

```powershell
pnpm verify
```
