# Melomae

Melomae is a free, open-source desktop app. It restores a music collection
that you already own. You add folders. Melomae shows which files can gain
from restoration and restores them on your own machine. It keeps every tag
and picture, and it mirrors your folder tree. No audio leaves your machine.

Melomae produces a reconstructed approximation. It can't recover the data
that lossy compression discarded, but it's probably good enough for your ears.

## Prerequisites

- Node.js 24 and pnpm 12.6.0. The `packageManager` field pins pnpm.
- Rust 1.95 or later. On Windows, use the MSVC toolchain.
- Python 3.10 or later.
- uv 0.12 or later.
- On Linux, the WebKitGTK 4.1 development packages.

## Setup

```powershell
pnpm install
```

## Development commands

| Command | Purpose |
| --- | --- |
| `pnpm dev` | Run the Vite dev server. |
| `pnpm tauri dev` | Run the desktop app in development. |
| `pnpm build` | Typecheck and build the frontend. |
| `pnpm typecheck` | Typecheck the frontend. |
| `pnpm lint` | Lint the frontend. |
| `pnpm test` | Run the frontend tests. |
| `pnpm verify` | Run the full gate. |
| `pnpm tauri build` | Build the desktop installer. |

## The verify gate

`pnpm verify` runs these checks in order:

1. `python tasks/validate_board.py`
2. `python -m unittest discover -s tasks -p "test_*.py"`
3. Frontend typecheck, lint, Vitest, and build
4. `cargo fmt --check`
5. `cargo clippy --workspace -- -D warnings`
6. `cargo test --workspace`
7. `uv run --project tools pytest`

## Repository layout

| Path | Contents |
| --- | --- |
| `crates/melomae-core` | The pipeline. It must not depend on any `tauri` crate. |
| `crates/melomae-cli` | A headless command line over the core. |
| `src` | The React frontend. |
| `src-tauri` | The Tauri app. It holds commands, events, and packaging only. |
| `tools` | The uv Python project. |
| `docs` | The product requirements and the stack. |
| `tasks` | The task board. |
| `.vendored` | The pinned agent skills and their sync scripts. |
| `.agents/skills`, `.claude/skills` | The installed copies of those skills. |

## Agent skills

The repository pins two agent skills as shallow git submodules under `.vendored`:

| Skill | Submodule | Upstream | Use |
| --- | --- | --- | --- |
| impeccable | `.vendored/impeccable` | <https://github.com/pbakaus/impeccable> | Design work and UI audits. |
| ponytail | `.vendored/ponytail` | <https://github.com/DietrichGebert/ponytail> | The smallest change that solves the task, and code audits. |

The installed copies are in `.agents/skills` and `.claude/skills`. Both are
committed, so a clone without submodules still has the skills. Do not edit a
copy by hand.

Each skill has its own sync script next to its submodule. The script replaces
that skill's folders in `.agents/skills` and `.claude/skills` with the pinned
upstream files. Impeccable ships a separate Claude build, and the script copies
that build to `.claude/skills`.

Copy the pinned commit again:

```powershell
git submodule update --init --depth 1 .vendored/ponytail
pwsh -File .vendored/ponytail-sync.ps1
```

Move the pin to the upstream tip and copy:

```powershell
pwsh -File .vendored/ponytail-sync.ps1 -Update
```

Use `impeccable` in place of `ponytail` for the other skill. On Linux or macOS,
run `bash .vendored/ponytail-sync.sh`, with `--update` to move the pin. Read the
diff, then commit the submodule pointer and both copies in one commit.

Keep `--depth 1`. A full clone of impeccable downloads more than 850 MB.

## License

Melomae is licensed under the GNU General Public License, version 3 only.
Some files in this repository use other licenses. README.md lists them.
Copyright (C) 2026 Rene Andre Bedonia Jocsing. The full text is in `LICENSE`.

Files in this repository use these licenses:

| Path | License |
| --- | --- |
| The Melomae code | GPL-3.0-only |
| `models/apollo.onnx` | CC BY-SA 4.0 |
| `tools/export/vendor/` | CC BY-SA 4.0 |
| `fixtures/ori_wav.wav` | CC BY-SA 4.0 |
| `tools/parity/input_wav.wav` | CC BY-SA 4.0 |
| `.vendored/impeccable` and the `impeccable` skill copies | Apache-2.0 |
| `.vendored/ponytail` and the `ponytail*` skill copies | MIT |

`crates/melomae-core/src/inference/mod.rs` and `tools/export/real_dsp.py` carry
the Apollo attribution.

## Attribution

Melomae uses [Apollo](https://github.com/JusperLee/Apollo) under the hood:

```bibtex
@inproceedings{li2025apollo,
  title={Apollo: Band-sequence Modeling for High-Quality Music Restoration in Compressed Audio},
  author={Li, Kai and Luo, Yi},
  booktitle={IEEE International Conference on Acoustics, Speech and Signal Processing (ICASSP)},
  year={2025},
  organization={IEEE}
}
```

Apollo's code and weights are licensed CC BY-SA 4.0.
Melomae converts the model to ONNX. `docs/melomae-stack.md`
records the pinned revision and the checkpoint checksum.
