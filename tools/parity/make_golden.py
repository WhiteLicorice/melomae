"""Build the task 04 parity golden with the stock Apollo inference script.

This is a development tool. The verify gate does not run it. Run it once, in the
conda environment that holds torch, with byte-code writing off so the Apollo
clone stays read-only:

    set PYTHONDONTWRITEBYTECODE=1
    conda run -n look2hear_win python tools/parity/make_golden.py

The tool builds an 18 s stereo input from three copies of the committed input
``tools/parity/input_wav.wav``, runs the stock ``C:\\Lab\\Apollo\\inference.py``
on it with the task 04 chunk settings, and writes ``golden.wav`` beside this
file with its SHA-256 in ``golden.wav.sha256``.

The input build matches the Rust parity test: both read the source WAV to
44.1 kHz float frames and concatenate the frames three times. The source is a
32-bit float stereo WAV, so both sides read the identical sample values.

The module imports torch, numpy, and soundfile only inside ``build_input`` and
``run_stock``. A pytest test can import the module without the export group.
"""

import argparse
import hashlib
import os
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent

DEFAULT_SOURCE = HERE / "input_wav.wav"
DEFAULT_INFERENCE = Path(r"C:\Lab\Apollo\inference.py")
DEFAULT_CHECKPOINT = Path(r"C:\Lab\Apollo\Apollo\pytorch_model.bin")
DEFAULT_OUTPUT = HERE / "golden.wav"

SAMPLE_RATE = 44_100
COPIES = 3


def sha256_file(path):
    """The SHA-256 of a file, streamed in blocks."""
    digest = hashlib.sha256()
    with Path(path).open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def build_input(source, target, copies=COPIES):
    """Write ``copies`` end-to-end copies of ``source`` to ``target``."""
    import numpy as np
    import soundfile as sf

    samples, rate = sf.read(source, dtype="float32", always_2d=True)
    if rate != SAMPLE_RATE:
        raise ValueError(f"the source must be {SAMPLE_RATE} Hz, got {rate} Hz")
    tiled = np.tile(samples, (copies, 1))
    sf.write(target, tiled, rate, subtype="FLOAT")
    return tiled.shape


def run_stock(inference, input_path, output_path, checkpoint, chunk_seconds, overlap_seconds, pad_seconds):
    """Run the stock Apollo inference script on the input, and return the command."""
    command = [
        sys.executable,
        str(inference),
        "--in_wav",
        str(input_path),
        "--out_wav",
        str(output_path),
        "--checkpoint",
        str(checkpoint),
        "--device",
        "cpu",
        "--chunk-seconds",
        str(chunk_seconds),
        "--overlap-seconds",
        str(overlap_seconds),
        "--chunk-pad-seconds",
        str(pad_seconds),
    ]
    environment = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")
    subprocess.run(command, check=True, cwd=str(Path(inference).parent), env=environment)
    return command


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--inference", type=Path, default=DEFAULT_INFERENCE)
    parser.add_argument("--checkpoint", type=Path, default=DEFAULT_CHECKPOINT)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--chunk-seconds", type=float, default=6.0)
    parser.add_argument("--overlap-seconds", type=float, default=1.0)
    parser.add_argument("--chunk-pad-seconds", type=float, default=1.0)
    arguments = parser.parse_args(argv)

    for label, path in (
        ("source", arguments.source),
        ("inference", arguments.inference),
        ("checkpoint", arguments.checkpoint),
    ):
        if not Path(path).is_file():
            print(f"the {label} is missing: {path}", file=sys.stderr)
            return 1

    source_sidecar = Path(str(arguments.source) + ".sha256")
    if source_sidecar.is_file():
        expected = source_sidecar.read_text(encoding="utf-8").strip().lower()
        actual = sha256_file(arguments.source)
        if actual != expected:
            print(
                f"the source does not match {source_sidecar}: {actual}",
                file=sys.stderr,
            )
            return 1

    with tempfile.TemporaryDirectory(prefix="melomae-parity-") as directory:
        input_path = Path(directory) / "input_18s.wav"
        shape = build_input(arguments.source, input_path)
        print(f"input: {input_path} {shape} (frames, channels)")
        command = run_stock(
            arguments.inference,
            input_path,
            arguments.output,
            arguments.checkpoint,
            arguments.chunk_seconds,
            arguments.overlap_seconds,
            arguments.chunk_pad_seconds,
        )
        print("command: " + " ".join(command))

    digest = sha256_file(arguments.output)
    sidecar = Path(str(arguments.output) + ".sha256")
    sidecar.write_text(digest + "\n", encoding="utf-8")
    print(f"golden: {arguments.output}")
    print(f"bytes: {Path(arguments.output).stat().st_size}")
    print(f"sha256: {digest}")
    print(f"sidecar: {sidecar}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
