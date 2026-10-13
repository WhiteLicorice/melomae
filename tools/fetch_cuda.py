#!/usr/bin/env python3
"""Download the CUDA ONNX Runtime build and the NVIDIA CUDA libraries into `.cache/cuda/`.

The GPU restore path (task 08) loads an ONNX Runtime build that carries the CUDA
execution provider, and that provider needs the NVIDIA CUDA and cuDNN runtime
libraries beside it. The engine never links these libraries; it loads them at run
time from a folder the caller gives (`melomae_core::inference::gpu_provider_available`
and `load_onnx_backend_on_device`). This script fetches that folder.

    python tools/fetch_cuda.py

The install lands in `.cache/cuda/<tag>/lib/`, where every dynamic library sits in
one flat folder: `onnxruntime.dll`, its `onnxruntime_providers_cuda.dll` and
`onnxruntime_providers_shared.dll`, and the CUDA and cuDNN libraries. The flat
folder is what the engine preloads, so the CUDA provider finds its dependencies.

Two pins:

1. The Microsoft ONNX Runtime release whose C API matches the pinned `ort` crate,
   built for CUDA 12.x. `ort` 2.0.0-rc.13 uses ONNX Runtime 1.28 (ORT API 27). The
   release asset is `onnxruntime-win-x64-gpu_cuda12-1.28.3.zip`.
2. The NVIDIA Python wheels that carry the CUDA and cuDNN runtime libraries for
   Windows. The ONNX Runtime CUDA execution provider page states the CUDA 12.x
   build needs CUDA 12.8 or newer and cuDNN 9.x:
   <https://onnxruntime.ai/docs/execution-providers/CUDA-ExecutionProvider.html>,
   table "CUDA 12.x", checked 2026-10-14. The pinned wheels are CUDA 12.9 and
   cuDNN 9.27.

The wheels and the release asset publish a SHA-256. The script checks the SHA-256
of every archive before it extracts, and it leaves no partial folder when a check
fails. It is idempotent: when the pinned build is already installed it does
nothing. Standard library only. It never writes outside `.cache/cuda`.

The app downloads these files itself at run time (a later task); this script is for
the tests and the measurements.
"""

from __future__ import annotations

import hashlib
import shutil
import sys
import tempfile
import urllib.request
import zipfile
from pathlib import Path

# The CUDA major the pinned `ort` crate preloads on Windows
# (`ort::ep::cuda::CUDA_DYLIBS`: cudart64_12, cublasLt64_12, cublas64_12, cufft64_11).
CUDA_MAJOR = 12

# The Microsoft GPU build whose C API matches the pinned `ort` version.
ORT_TAG = "v1.28.3"
ORT_ASSET = "onnxruntime-win-x64-gpu_cuda12-1.28.3.zip"
ORT_SHA256 = "65d7f956649df80a8ccdcafa7843e4b02f38bfc9e6de28711ed8384508040330"
ORT_URL = f"https://github.com/microsoft/onnxruntime/releases/download/{ORT_TAG}/{ORT_ASSET}"

# The NVIDIA runtime wheels for Windows x86_64. Each entry is
# (wheel file name, download URL, SHA-256). The hashes come from the PyPI JSON
# API, `https://pypi.org/pypi/<name>/<version>/json`.
NVIDIA_WHEELS = [
    (
        "nvidia_cuda_runtime_cu12-12.9.79-py3-none-win_amd64.whl",
        "https://files.pythonhosted.org/packages/59/df/e7c3a360be4f7b93cee39271b792669baeb3846c58a4df6dfcf187a7ffab/"
        "nvidia_cuda_runtime_cu12-12.9.79-py3-none-win_amd64.whl",
        "8e018af8fa02363876860388bd10ccb89eb9ab8fb0aa749aaf58430a9f7c4891",
    ),
    (
        "nvidia_cublas_cu12-12.9.2.10-py3-none-win_amd64.whl",
        "https://files.pythonhosted.org/packages/20/e2/fc9a0e985249d873150276d5afb02e39a66817fedbf1a385724393e505ed/"
        "nvidia_cublas_cu12-12.9.2.10-py3-none-win_amd64.whl",
        "623f43027d40d44ceadf0043f002bd25cf353e8f13ce90b9a87057019f560661",
    ),
    (
        "nvidia_cufft_cu12-11.4.1.4-py3-none-win_amd64.whl",
        "https://files.pythonhosted.org/packages/20/ee/29955203338515b940bd4f60ffdbc073428f25ef9bfbce44c9a066aedc5c/"
        "nvidia_cufft_cu12-11.4.1.4-py3-none-win_amd64.whl",
        "8e5bfaac795e93f80611f807d42844e8e27e340e0cde270dcb6c65386d795b80",
    ),
    (
        "nvidia_curand_cu12-10.3.10.19-py3-none-win_amd64.whl",
        "https://files.pythonhosted.org/packages/e5/98/1bd66fd09cbe1a5920cb36ba87029d511db7cca93979e635fd431ad3b6c0/"
        "nvidia_curand_cu12-10.3.10.19-py3-none-win_amd64.whl",
        "e8129e6ac40dc123bd948e33d3e11b4aa617d87a583fa2f21b3210e90c743cde",
    ),
    (
        "nvidia_cudnn_cu12-9.27.0.42-py3-none-win_amd64.whl",
        "https://files.pythonhosted.org/packages/aa/38/f856579877f7c1c5066e61182e7de7bc27bf35a78c8d1b0fa592e6985bc4/"
        "nvidia_cudnn_cu12-9.27.0.42-py3-none-win_amd64.whl",
        "06e9b0026f3bad97d2b58666330fabec04fe1672f776661ecb0ce0029c27f142",
    ),
]

REPO_ROOT = Path(__file__).resolve().parents[1]
TAG = f"{ORT_TAG}-cuda{CUDA_MAJOR}"
TAG_DIR = REPO_ROOT / ".cache" / "cuda" / TAG
ARCHIVE_DIR = REPO_ROOT / ".cache" / "cuda" / "archives"
COMPLETE_MARKER = ".complete"

# A browser user agent. GitHub throttles the default `Python-urllib` agent, so an
# unset agent makes the download about an order of magnitude slower.
USER_AGENT = "melomae-fetch-cuda/1"

# The library the engine loads, and the CUDA provider library it needs.
RUNTIME_NAME = "onnxruntime.dll"
PROVIDER_NAME = "onnxruntime_providers_cuda.dll"

# On Windows the libraries have these suffixes; the checks below are Windows-only.
SUFFIXES = (".dll",)


class ChecksumError(RuntimeError):
    """The downloaded archive does not match the pinned SHA-256."""


def sha256_of(path: Path) -> str:
    """The hex SHA-256 of a file."""
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def download(url: str) -> bytes:
    """Fetch a URL. Separated out so a test can replace it."""
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(request) as response:  # noqa: S310 - pinned https URL
        return response.read()


def fetch_archive(file_name: str, url: str, expected_sha256: str, *, fetch=download) -> Path:
    """Return the pinned archive, downloading it into the archive cache on first use.

    An archive whose SHA-256 already matches is reused, so a re-run after a failure
    does not download gigabytes again. A mismatch removes the file, so a partial or
    corrupt archive is never reused.
    """
    ARCHIVE_DIR.mkdir(parents=True, exist_ok=True)
    target = ARCHIVE_DIR / file_name
    if target.is_file() and sha256_of(target) == expected_sha256:
        return target
    target.write_bytes(fetch(url))
    digest = sha256_of(target)
    if digest != expected_sha256:
        target.unlink(missing_ok=True)
        raise ChecksumError(f"{file_name}: sha256 {digest} does not match {expected_sha256}")
    return target


def is_complete(tag_dir: Path) -> bool:
    """True when the tag directory holds a finished install."""
    return (tag_dir / COMPLETE_MARKER).is_file() and (tag_dir / "lib" / RUNTIME_NAME).is_file()


def extract_libraries(archive: Path, dest: Path) -> None:
    """Extract every dynamic library of a `.zip` archive into `dest`, flattened.

    The ONNX Runtime release keeps its libraries under `<name>/lib/`. A wheel keeps
    the NVIDIA libraries under `nvidia/<component>/bin/`. Both are flattened to the
    bare file name, so the engine finds every library in one folder. Only members
    under a `lib/` or `bin/` directory are read; a wheel's headers and metadata are
    skipped.
    """
    with zipfile.ZipFile(archive) as handle:
        for member in handle.infolist():
            if member.is_dir():
                continue
            parts = Path(member.filename).parts
            name = parts[-1]
            if not name.lower().endswith(SUFFIXES):
                continue
            if not any(part.lower() in ("lib", "bin") for part in parts[:-1]):
                continue
            with handle.open(member) as source:
                (dest / name).write_bytes(source.read())


def install_all(archives: list[tuple[str, str, str, str]], *, fetch=download) -> Path:
    """Install every pinned archive into `tag_dir`, or return when it is complete.

    `archives` is a list of `(file name, url, sha256, kind)`. The work happens in a
    staging directory next to `tag_dir`, so a checksum failure removes the staging
    directory and leaves no partial install.
    """
    if is_complete(TAG_DIR):
        return TAG_DIR

    TAG_DIR.parent.mkdir(parents=True, exist_ok=True)
    staging = Path(tempfile.mkdtemp(prefix=f"{TAG_DIR.name}-", dir=TAG_DIR.parent))
    try:
        content = staging / "content"
        unpacked = content / "lib"
        unpacked.mkdir(parents=True)
        for file_name, url, expected_sha256, _kind in archives:
            archive = fetch_archive(file_name, url, expected_sha256, fetch=fetch)
            extract_libraries(archive, unpacked)

        for required in (RUNTIME_NAME, PROVIDER_NAME):
            if not (unpacked / required).is_file():
                raise RuntimeError(f"the pinned archives did not provide {required}")

        marker = "".join(f"{name} {sha}\n" for name, _url, sha, _kind in archives)
        (content / COMPLETE_MARKER).write_text(marker, encoding="utf-8")

        if TAG_DIR.exists():
            shutil.rmtree(TAG_DIR)
        shutil.move(str(content), str(TAG_DIR))
    finally:
        shutil.rmtree(staging, ignore_errors=True)

    return TAG_DIR


def assets() -> list[tuple[str, str, str, str]]:
    """The pinned archives: the ONNX Runtime build first, then the NVIDIA wheels."""
    entries = [(ORT_ASSET, ORT_URL, ORT_SHA256, "onnxruntime")]
    entries.extend((name, url, sha, "nvidia") for name, url, sha in NVIDIA_WHEELS)
    return entries


def main() -> int:
    if not sys.platform.startswith("win"):
        raise SystemExit(f"the pinned CUDA libraries are Windows-only: {sys.platform}")
    tag_dir = install_all(assets())
    library = tag_dir / "lib" / RUNTIME_NAME
    print(f"cuda onnxruntime ready: {library}")
    print(f"cuda libraries: {tag_dir / 'lib'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
