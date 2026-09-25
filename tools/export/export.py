"""Export the vendored Apollo model to ONNX with real-arithmetic DSP.

Run as a script:

    uv run --project tools python tools/export/export.py

The script loads the pinned Apollo checkpoint, wraps the model with the
real-arithmetic STFT and iSTFT in ``real_dsp.py``, and writes ``apollo.onnx``
with a dynamic sample axis. See ``vendor/PROVENANCE.md`` for the source and
the license.
"""

import argparse
import hashlib
import json
import os
import sys
from pathlib import Path

import torch

HERE = Path(__file__).resolve().parent
# Import the wrapper by its package path on every entry path. The exporter
# writes the module path into node metadata, so a script-relative import
# changes the ONNX file bytes.
if str(HERE.parent) not in sys.path:
    sys.path.insert(0, str(HERE.parent))

from export.real_dsp import RealApollo  # noqa: E402

VENDOR = HERE / "vendor"
CACHE = HERE / ".cache"
if str(VENDOR) not in sys.path:
    sys.path.insert(0, str(VENDOR))

from look2hear.models.apollo import Apollo  # noqa: E402

DEFAULT_OUTPUT = HERE / "artifacts" / "apollo.onnx"
DEFAULT_SAMPLE_RATE = 44100

HF_REPO = "JusperLee/Apollo"
HF_REVISION = "c68bd80fdd9c0d93d2f4a833cb154624f660a561"
CHECKPOINT_NAME = "pytorch_model.bin"
CHECKPOINT_SHA256 = "99d9af7f1ff20e63c393035513a655392818d66b4d7fc23d658175c1f15e8d76"
CHECKPOINT_BYTES = 66541845


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def ensure_checkpoint(path=None) -> Path | None:
    """Return a verified checkpoint path, or ``None`` when none is available."""
    candidates = []
    if path is not None:
        candidates.append(Path(path))
    env_path = os.environ.get("MELOMAE_APOLLO_CHECKPOINT")
    if env_path:
        candidates.append(Path(env_path))
    candidates.append(CACHE / CHECKPOINT_NAME)
    candidates.append(Path(r"C:\Lab\Apollo\Apollo\pytorch_model.bin"))
    for candidate in candidates:
        if candidate.is_file() and sha256_file(candidate) == CHECKPOINT_SHA256:
            return candidate

    try:
        from huggingface_hub import hf_hub_download

        CACHE.mkdir(parents=True, exist_ok=True)
        downloaded = Path(
            hf_hub_download(
                repo_id=HF_REPO,
                filename=CHECKPOINT_NAME,
                revision=HF_REVISION,
                local_dir=str(CACHE),
            )
        )
    except Exception as error:  # Network, auth, or hub failure.
        print(f"Download failed: {error}", file=sys.stderr)
        return None
    if downloaded.is_file() and sha256_file(downloaded) == CHECKPOINT_SHA256:
        return downloaded
    return None


def load_apollo(checkpoint: Path) -> Apollo:
    payload = torch.load(checkpoint, map_location="cpu", weights_only=False)
    model = Apollo(**payload["model_args"])
    model.load_state_dict(payload["state_dict"])
    model.eval()
    for parameter in model.parameters():
        parameter.requires_grad_(False)
    return model


def build_wrapper(apollo: Apollo) -> RealApollo:
    wrapper = RealApollo(apollo).eval()
    for parameter in wrapper.parameters():
        parameter.requires_grad_(False)
    return wrapper


def export_to(output: Path, checkpoint: Path | None = None, opset: int | None = None) -> dict:
    """Export ``apollo.onnx`` and return a manifest of the result."""
    if checkpoint is None:
        checkpoint = ensure_checkpoint()
    if checkpoint is None:
        raise FileNotFoundError("No verified Apollo checkpoint is available.")
    apollo = load_apollo(checkpoint)
    wrapper = build_wrapper(apollo)

    dummy = torch.zeros(1, 2, DEFAULT_SAMPLE_RATE, dtype=torch.float32)
    output.parent.mkdir(parents=True, exist_ok=True)
    export_kwargs = {
        "dynamo": True,
        "verbose": False,
        "input_names": ["audio"],
        "output_names": ["audio_out"],
        "dynamic_shapes": (
            {
                0: torch.export.Dim("batch"),
                1: torch.export.Dim("channels"),
                2: torch.export.Dim("samples", min=2 * 882),
            },
        ),
        "do_constant_folding": True,
        "external_data": False,
    }
    if opset is not None:
        export_kwargs["opset_version"] = opset
    torch.onnx.export(wrapper, (dummy,), str(output), **export_kwargs)
    return describe_onnx(output)


def describe_onnx(path: Path) -> dict:
    import onnx

    model = onnx.load(str(path))
    opset = {entry.domain or "ai.onnx": entry.version for entry in model.opset_import}
    return {
        "path": str(path),
        "bytes": path.stat().st_size,
        "sha256": sha256_file(path),
        "opset": opset,
        "ir_version": model.ir_version,
        "nodes": len(model.graph.node),
        "initializers": len(model.graph.initializer),
    }


def build_onnx_session(path: Path, intra_op_threads: int | None = None):
    import onnxruntime as ort

    options = ort.SessionOptions()
    options.intra_op_num_threads = int(
        intra_op_threads if intra_op_threads is not None else physical_cores()
    )
    options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
    return ort.InferenceSession(
        str(path), sess_options=options, providers=["CPUExecutionProvider"]
    )


def physical_cores() -> int:
    try:
        import psutil

        count = psutil.cpu_count(logical=False)
        if count:
            return int(count)
    except ImportError:
        pass
    return os.cpu_count() or 1


def main() -> int:
    import onnxruntime as ort

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--checkpoint", type=Path, default=None)
    parser.add_argument("--opset", type=int, default=None)
    arguments = parser.parse_args()

    checkpoint = ensure_checkpoint(arguments.checkpoint)
    if checkpoint is None:
        print("No verified Apollo checkpoint is available.", file=sys.stderr)
        return 1
    manifest = export_to(arguments.output, checkpoint=checkpoint, opset=arguments.opset)
    manifest["onnxruntime"] = ort.__version__
    manifest["torch"] = torch.__version__
    manifest["intra_op_threads"] = physical_cores()
    print(json.dumps(manifest, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
