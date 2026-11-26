"""Tests for the CUDA ONNX Runtime fetch script.

These tests never touch the network.
"""

from __future__ import annotations

import importlib.util
import zipfile
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "fetch_cuda.py"


def load_module():
    spec = importlib.util.spec_from_file_location("fetch_cuda", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def test_the_pin_names_the_microsoft_cuda_12_gpu_nuget_package() -> None:
    module = load_module()
    version = module.ORT_TAG.lstrip("v")
    assert module.ORT_ASSET == f"microsoft.ml.onnxruntime.gpu.windows.{version}.nupkg"
    # From 1.27 the nuget.org GPU package is a CUDA 13 build. The CUDA 12 build that
    # the pinned NVIDIA wheels and `ort`'s CUDA 12 preload match is on Microsoft's
    # `onnxruntime-cuda-12` feed (task 62).
    assert module.ORT_URL == (
        "https://aiinfra.pkgs.visualstudio.com/PublicPackages/_packaging/onnxruntime-cuda-12/"
        f"nuget/v3/flat2/microsoft.ml.onnxruntime.gpu.windows/{version}/{module.ORT_ASSET}"
    )
    assert version.startswith("1.28")
    assert module.CUDA_MAJOR == 12


def test_the_win_x64_libraries_of_a_nuget_package_are_flattened(tmp_path: Path) -> None:
    module = load_module()
    package = tmp_path / "package.nupkg"
    with zipfile.ZipFile(package, "w") as handle:
        handle.writestr("runtimes/win-x64/native/onnxruntime.dll", "x64 runtime")
        handle.writestr("runtimes/win-x64/native/onnxruntime_providers_cuda.dll", "x64 cuda")
        handle.writestr("runtimes/win-x64/native/onnxruntime.lib", "import library")
        handle.writestr("runtimes/win-arm64/native/onnxruntime.dll", "arm64 runtime")
    dest = tmp_path / "lib"
    dest.mkdir()

    module.extract_libraries(package, dest)

    assert sorted(path.name for path in dest.iterdir()) == [
        "onnxruntime.dll",
        "onnxruntime_providers_cuda.dll",
    ]
    # Another platform's library of the same name must never replace the x64 one.
    assert (dest / "onnxruntime.dll").read_text(encoding="utf-8") == "x64 runtime"
