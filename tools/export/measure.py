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
    if sys.platform == "win32":
        import ctypes
        from ctypes import wintypes

        class Counters(ctypes.Structure):
            _fields_ = [
                ("cb", wintypes.DWORD),
                ("PageFaultCount", wintypes.DWORD),
                ("PeakWorkingSetSize", ctypes.c_size_t),
                ("WorkingSetSize", ctypes.c_size_t),
                ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
                ("QuotaPagedPoolUsage", ctypes.c_size_t),
                ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
                ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
                ("PagefileUsage", ctypes.c_size_t),
                ("PeakPagefileUsage", ctypes.c_size_t),
            ]

        counters = Counters()
        counters.cb = ctypes.sizeof(counters)
        ctypes.windll.psapi.GetProcessMemoryInfo(
            ctypes.windll.kernel32.GetCurrentProcess(),
            ctypes.byref(counters),
            counters.cb,
        )
        return int(counters.PeakWorkingSetSize)
    import resource

    return int(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss * 1024)


def run_ort_once(seconds: float) -> dict:
    session = build_onnx_session(ARTIFACT)
    audio = load_segment(seconds)
    session.run(None, {"audio": audio.numpy()})
    return {"seconds": seconds, "peak_working_set_bytes": peak_working_set_bytes()}


def time_ort(session, audio: torch.Tensor) -> float:
    start = time.perf_counter()
    session.run(None, {"audio": audio.numpy()})
    return time.perf_counter() - start


def time_torch(model, audio: torch.Tensor) -> float:
    with torch.no_grad():
        start = time.perf_counter()
        model(audio)
        return time.perf_counter() - start


def measure(checkpoint: Path) -> dict:
    torch.set_num_threads(physical_cores())
    threads = physical_cores()
    apollo = load_apollo(checkpoint)
    session = build_onnx_session(ARTIFACT)

    results: dict = {"intra_op_threads": threads, "sample_rate": SAMPLE_RATE}

    warm = load_segment(1.0)
    time_torch(apollo, warm)
    session.run(None, {"audio": warm.numpy()})

    realtime = {}
    for seconds in (1.0, 3.0, 6.0):
        audio = load_segment(seconds)
        torch_seconds = time_torch(apollo, audio)
        ort_seconds = time_ort(session, audio)
        realtime[str(seconds)] = {
            "torch_rtf": torch_seconds / seconds,
            "onnx_rtf": ort_seconds / seconds,
            "torch_wall_seconds": torch_seconds,
            "onnx_wall_seconds": ort_seconds,
        }
    results["realtime"] = realtime

    audio = load_segment(6.0)
    with torch.no_grad():
        stock = apollo(audio).numpy()
    onnx = session.run(None, {"audio": audio.numpy()})[0]
    difference = np.abs(stock - onnx)
    results["parity_6s"] = {
        "max_abs": float(difference.max()),
        "rmse": float(np.sqrt(np.mean(difference**2))),
    }

    stereo = session.run(None, {"audio": audio.numpy()})[0]
    left = session.run(None, {"audio": audio[:, :1].numpy()})[0]
    right = session.run(None, {"audio": audio[:, 1:].numpy()})[0]
    results["channel_independence_6s"] = {
        "left_max_abs": float(np.abs(stereo[:, :1] - left).max()),
        "right_max_abs": float(np.abs(stereo[:, 1:] - right).max()),
    }

    peaks = {}
    for seconds in (2.0, 6.0):
        raw = subprocess.run(
            [sys.executable, str(Path(__file__).resolve()), "--peak-seconds", str(seconds)],
            capture_output=True,
            text=True,
            check=True,
        )
        peaks[str(seconds)] = json.loads(raw.stdout)
    slope = (peaks["6.0"]["peak_working_set_bytes"] - peaks["2.0"]["peak_working_set_bytes"]) / (
        (6.0 - 2.0) * CHANNELS
    )
    results["peak_working_set"] = {
        "at_2s_bytes": peaks["2.0"]["peak_working_set_bytes"],
        "at_6s_bytes": peaks["6.0"]["peak_working_set_bytes"],
        "bytes_per_channel_second": slope,
    }
    return results


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--peak-seconds", type=float, default=None, help=argparse.SUPPRESS)
    arguments = parser.parse_args()
    if arguments.peak_seconds is not None:
        ARTIFACT.parent.mkdir(parents=True, exist_ok=True)
        print(json.dumps(run_ort_once(arguments.peak_seconds)))
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
    report.update(measure(checkpoint))

    replay = Path(r"C:\Users\Ren\AppData\Local\Temp\opencode\apollo_replay.onnx")
    export_to(replay, checkpoint=checkpoint)
    report["determinism"] = {
        "first_sha256": sha256_file(ARTIFACT),
        "second_sha256": sha256_file(replay),
        "identical": sha256_file(ARTIFACT) == sha256_file(replay),
    }
    print(json.dumps(report, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
