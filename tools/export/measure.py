"""Measure the ONNX Apollo export against stock torch.

Run as a script:

    uv run --project tools python tools/export/measure.py

The script reports parity, real-time factor, channel independence, peak
working set, and export determinism for ``docs/evidence/01-onnx-spike.md``.
Each measurement runs on ``C:\\Lab\\Apollo\\asserts\\input_wav.wav``.
"""

import argparse
import json
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import numpy as np
import soundfile as sf
import torch

HERE = Path(__file__).resolve().parent
TOOLS = HERE.parent
if str(TOOLS) not in sys.path:
    sys.path.insert(0, str(TOOLS))

from export.export import (  # noqa: E402
    build_onnx_session,
    ensure_checkpoint,
    export_to,
    load_apollo,
    physical_cores,
    sha256_file,
)

SAMPLE_RATE = 44100
CHANNELS = 2
WAV = Path(r"C:\Lab\Apollo\asserts\input_wav.wav")
ARTIFACT = HERE / "artifacts" / "apollo.onnx"


def load_segment(seconds: float) -> torch.Tensor:
    audio, _ = sf.read(str(WAV), dtype="float32", always_2d=True)
    count = int(round(seconds * SAMPLE_RATE))
    segment = audio[:count].T
    return torch.from_numpy(segment[None].astype(np.float32))


def peak_working_set_bytes() -> int:
    """Return the peak resident memory of this process, in bytes."""
    if sys.platform == "win32":
        import psutil

        return int(psutil.Process().memory_info().peak_wset)
    import resource

    return int(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss * 1024)


def run_ort_once(seconds: float, channels: int = CHANNELS) -> dict:
    baseline = peak_working_set_bytes()
    session = build_onnx_session(ARTIFACT)
    audio = load_segment(seconds)[:, :channels]
    loaded = peak_working_set_bytes()
    session.run(None, {"audio": audio.numpy()})
    return {
        "seconds": seconds,
        "channels": channels,
        "before_session_bytes": baseline,
        "after_session_bytes": loaded,
        "peak_working_set_bytes": peak_working_set_bytes(),
    }


def time_ort(session, audio: torch.Tensor) -> float:
    start = time.perf_counter()
    session.run(None, {"audio": audio.numpy()})
    return time.perf_counter() - start


def time_torch(model, audio: torch.Tensor) -> float:
    with torch.no_grad():
        start = time.perf_counter()
        model(audio)
        return time.perf_counter() - start


def measure_realtime(apollo, session, rounds: int, channels: int) -> dict:
    """Time torch and ONNX Runtime in alternating rounds and report medians.

    Each round runs both engines once. The order flips every round, so a
    thermal or background-load drift affects both engines equally. The
    ratio is the median of the per-round ratios.

    Stereo at 6 s pages on a 15 GB machine, because the ONNX Runtime arena
    keeps its 7 GB peak while torch allocates. Mono is the shipped path
    (Stack §C) and does not page.
    """
    realtime = {}
    for seconds in (1.0, 3.0, 6.0):
        audio = load_segment(seconds)[:, :channels]
        torch_times, ort_times = [], []
        for index in range(rounds):
            if index % 2 == 0:
                torch_times.append(time_torch(apollo, audio))
                ort_times.append(time_ort(session, audio))
            else:
                ort_times.append(time_ort(session, audio))
                torch_times.append(time_torch(apollo, audio))
        ratios = [o / t for o, t in zip(ort_times, torch_times)]
        realtime[str(seconds)] = {
            "rounds": rounds,
            "torch_rtf_median": float(np.median(torch_times)) / seconds,
            "onnx_rtf_median": float(np.median(ort_times)) / seconds,
            "ratio_median": float(np.median(ratios)),
            "ratio_min": float(min(ratios)),
            "ratio_max": float(max(ratios)),
        }
    return realtime


def channel_independence(audio: torch.Tensor, threads: int) -> dict:
    session = build_onnx_session(ARTIFACT, intra_op_threads=threads)
    stereo = session.run(None, {"audio": audio.numpy()})[0]
    left = session.run(None, {"audio": audio[:, :1].numpy()})[0]
    right = session.run(None, {"audio": audio[:, 1:].numpy()})[0]
    return {
        "left_max_abs": float(np.abs(stereo[:, :1] - left).max()),
        "right_max_abs": float(np.abs(stereo[:, 1:] - right).max()),
    }


def shipped_path_parity(stock: np.ndarray, session, audio: torch.Tensor) -> dict:
    """Compare stock torch on stereo with ONNX Runtime on one channel at a time.

    The engine runs one channel at a time (Stack §C), so this is the output
    that ships.
    """
    left = session.run(None, {"audio": audio[:, :1].numpy()})[0]
    right = session.run(None, {"audio": audio[:, 1:].numpy()})[0]
    difference = np.abs(stock - np.concatenate([left, right], axis=1))
    return {
        "max_abs": float(difference.max()),
        "rmse": float(np.sqrt(np.mean(difference**2))),
    }


def peak_in_subprocess(seconds: float, channels: int) -> dict:
    raw = subprocess.run(
        [
            sys.executable,
            str(Path(__file__).resolve()),
            "--peak-seconds",
            str(seconds),
            "--peak-channels",
            str(channels),
        ],
        capture_output=True,
        text=True,
        check=True,
    )
    return json.loads(raw.stdout.strip().splitlines()[-1])


def measure(checkpoint: Path, rounds: int) -> dict:
    threads = physical_cores()
    torch.set_num_threads(threads)
    apollo = load_apollo(checkpoint)
    session = build_onnx_session(ARTIFACT)

    results: dict = {"intra_op_threads": threads, "sample_rate": SAMPLE_RATE}

    warm = load_segment(1.0)
    time_torch(apollo, warm)
    session.run(None, {"audio": warm.numpy()})
    results["realtime"] = {
        "mono": measure_realtime(apollo, session, rounds, 1),
        "stereo": measure_realtime(apollo, session, rounds, CHANNELS),
    }

    audio = load_segment(6.0)
    with torch.no_grad():
        stock = apollo(audio).numpy()
    onnx = session.run(None, {"audio": audio.numpy()})[0]
    difference = np.abs(stock - onnx)
    results["parity_6s"] = {
        "max_abs": float(difference.max()),
        "rmse": float(np.sqrt(np.mean(difference**2))),
    }

    results["channel_independence_6s"] = {
        str(count): channel_independence(audio, count) for count in sorted({1, 4, threads})
    }
    results["shipped_path_6s"] = shipped_path_parity(stock, session, audio)

    peaks = {}
    for channels in (1, CHANNELS):
        points = [peak_in_subprocess(seconds, channels) for seconds in (1.0, 2.0, 4.0, 6.0)]
        first, last = points[0], points[-1]
        peaks[f"{channels}ch"] = {
            "points": points,
            "bytes_per_channel_second": (
                last["peak_working_set_bytes"] - first["peak_working_set_bytes"]
            )
            / ((last["seconds"] - first["seconds"]) * channels),
        }
    results["peak_working_set"] = peaks
    return results


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rounds", type=int, default=7)
    parser.add_argument("--peak-seconds", type=float, default=None, help=argparse.SUPPRESS)
    parser.add_argument("--peak-channels", type=int, default=CHANNELS, help=argparse.SUPPRESS)
    arguments = parser.parse_args()
    if arguments.peak_seconds is not None:
        print(json.dumps(run_ort_once(arguments.peak_seconds, arguments.peak_channels)))
        return 0

    checkpoint = ensure_checkpoint()
    if checkpoint is None:
        print("No verified checkpoint.", file=sys.stderr)
        return 1

    report = {"checkpoint": str(checkpoint), "artifact": str(ARTIFACT)}
    report["artifact_metadata"] = {
        "bytes": ARTIFACT.stat().st_size,
        "sha256": sha256_file(ARTIFACT),
    }
    report.update(measure(checkpoint, arguments.rounds))

    with tempfile.TemporaryDirectory() as scratch:
        replay = Path(scratch) / "apollo_replay.onnx"
        export_to(replay, checkpoint=checkpoint)
        report["determinism"] = {
            "first_sha256": report["artifact_metadata"]["sha256"],
            "second_sha256": sha256_file(replay),
            "identical": report["artifact_metadata"]["sha256"] == sha256_file(replay),
        }
    print(json.dumps(report, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
