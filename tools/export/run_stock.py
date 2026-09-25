"""Run stock Apollo on the reference WAV and save the output.

Both the main tools environment (torch 2.11) and ``tools/oldtorch``
(torch 2.0.0) run this script. Compare the two outputs with ``--compare``:

    uv run --project tools/oldtorch python tools/export/run_stock.py --output old.npy
    uv run --project tools python tools/export/run_stock.py --output new.npy
    uv run --project tools python tools/export/run_stock.py --compare old.npy new.npy
"""

import argparse
import json
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
if str(HERE.parent) not in sys.path:
    sys.path.insert(0, str(HERE.parent))

WAV = Path(r"C:\Lab\Apollo\asserts\input_wav.wav")


def run(output: Path) -> None:
    import soundfile as sf
    import torch

    from export.export import ensure_checkpoint, load_apollo

    checkpoint = ensure_checkpoint()
    if checkpoint is None:
        raise FileNotFoundError("No verified Apollo checkpoint is available.")
    audio, _ = sf.read(str(WAV), dtype="float32", always_2d=True)
    model = load_apollo(checkpoint)
    with torch.no_grad():
        restored = model(torch.from_numpy(audio.T[None].copy()))
    np.save(output, restored.numpy())
    print(json.dumps({"torch": torch.__version__, "python": sys.version.split()[0]}))


def compare(first: Path, second: Path) -> None:
    difference = np.abs(np.load(first) - np.load(second))
    print(
        json.dumps(
            {
                "max_abs": float(difference.max()),
                "rmse": float(np.sqrt(np.mean(difference**2))),
            }
        )
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--compare", type=Path, nargs=2)
    arguments = parser.parse_args()
    if arguments.compare:
        compare(*arguments.compare)
    else:
        run(arguments.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
