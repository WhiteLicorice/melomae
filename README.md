# Melomae

Melomae is a free, open-source desktop app. It builds an offline music library.
The user picks music on MusicBrainz. Melomae finds the audio on YouTube Music,
downloads it, restores and repairs it, tags it, and files it into the library.
Melomae can also restore and repair audio files that the user already owns.

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

## License

Melomae is licensed under the GNU General Public License, version 3 or later.
Copyright (C) 2026 Rene Andre Bedonia Jocsing. The full text is in `LICENSE`.

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
