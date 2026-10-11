#!/usr/bin/env python3
"""Download the pinned ONNX Runtime build into `.cache/onnxruntime/<tag>/`.

The inference engine loads ONNX Runtime at run time from a library path that the caller
gives (`melomae_core::inference::load_onnx_backend`). The engine never links the runtime
statically. The tests and development use the library this script fetches.

    python tools/fetch_onnxruntime.py

This script holds one pin: the Microsoft ONNX Runtime release whose C API matches the
pinned `ort` crate. `ort` 2.0.0-rc.13 uses ONNX Runtime 1.28 (ORT API version 27), so the
pin is the `v1.28.3` patch of that minor line. The release assets:

    linux-x64  onnxruntime-linux-x64-1.28.3.tgz
    win-x64    onnxruntime-win-x64-1.28.3.zip

The Linux library keeps the glibc 2.35 baseline of SRS §2. A direct read of the ELF
version-need table of `libonnxruntime.so.1.28.3` shows its newest glibc requirement is
`GLIBC_2.27`, and it holds no `__isoc23_*` symbol. The glibc 2.35 of Ubuntu 22.04 is
therefore sufficient. The pyke.io binary that `ort`'s default features download is built
with a newer toolchain, and its `__isoc23_strtol`, `__isoc23_strtoll`, and
`__isoc23_strtoull` need glibc 2.38. That is why this script pins the Microsoft build.

It is idempotent: when the pinned build is already installed it does nothing. It checks
the SHA-256 before it extracts, and it leaves no partial folder when the check fails.
Standard library only. It never writes outside `.cache/onnxruntime`.
"""

from __future__ import annotations

import hashlib
import shutil
import sys
import tarfile
import tempfile
import urllib.request
import zipfile
from pathlib import Path

# One pin: a Microsoft ONNX Runtime release that matches the pinned `ort` API. The SHA-256
# values come from the release assets on GitHub. Confirm them with `gh release view v1.28.3`.
TAG = "v1.28.3"
ASSETS = {
    "win-x64": (
        "onnxruntime-win-x64-1.28.3.zip",
        "1d6fab48e85f948436af7c8c971d2c145cf224e2c444755dec894f8b0de11a83",
    ),
    "linux-x64": (
        "onnxruntime-linux-x64-1.28.3.tgz",
        "db14e4863bd37893fc59729d986ab2a0d043d10b7d44da1913c4982b7e3d009c",
    ),
}
BASE_URL = f"https://github.com/microsoft/onnxruntime/releases/download/{TAG}"
REPO_ROOT = Path(__file__).resolve().parents[1]
TAG_DIR = REPO_ROOT / ".cache" / "onnxruntime" / TAG
COMPLETE_MARKER = ".complete"


class ChecksumError(RuntimeError):
    """The downloaded asset does not match the pinned SHA-256."""


def platform_asset() -> tuple[str, str]:
    """The (asset name, sha256) for the current platform."""
    if sys.platform.startswith("win"):
        return ASSETS["win-x64"]
    if sys.platform.startswith("linux"):
        return ASSETS["linux-x64"]
    raise SystemExit(f"no pinned ONNX Runtime asset for this platform: {sys.platform}")


def sha256_of(path: Path) -> str:
    """The hex SHA-256 of a file."""
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def library(tag_dir: Path) -> Path | None:
    """The ONNX Runtime shared library inside an extracted tag directory, if it is there."""
    for pattern in ("*/lib/onnxruntime.dll", "*/lib/libonnxruntime.so", "*/lib/libonnxruntime.so.*"):
        for candidate in sorted(tag_dir.glob(pattern)):
            if candidate.is_file():
                return candidate
    return None


def is_complete(tag_dir: Path) -> bool:
    """True when the tag directory holds a finished install."""
    return (tag_dir / COMPLETE_MARKER).is_file() and library(tag_dir) is not None


def extract(archive: Path, dest: Path) -> None:
    """Extract a `.zip` or `.tgz` archive into `dest`."""
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as handle:
            handle.extractall(dest)
    else:
        with tarfile.open(archive) as handle:
            handle.extractall(dest, filter="data")


def download(url: str) -> bytes:
    """Fetch a URL. Separated out so the tests can replace it."""
    with urllib.request.urlopen(url) as response:  # noqa: S310 - pinned https URL
        return response.read()


def install(
    tag_dir: Path,
    asset_name: str,
    expected_sha256: str,
    *,
    url: str,
    fetch=download,
) -> Path:
    """Install the asset into `tag_dir`, or return it unchanged when it is already complete.

    The work happens in a staging directory next to `tag_dir`, so a failure (a checksum
    mismatch, for example) removes the staging directory and leaves no partial install.
    """
    if is_complete(tag_dir):
        return tag_dir

    tag_dir.parent.mkdir(parents=True, exist_ok=True)
    staging = Path(tempfile.mkdtemp(prefix=f"{tag_dir.name}-", dir=tag_dir.parent))
    try:
        archive = staging / asset_name
        archive.write_bytes(fetch(url))
        digest = sha256_of(archive)
        if digest != expected_sha256:
            raise ChecksumError(f"{asset_name}: sha256 {digest} does not match {expected_sha256}")

        unpacked = staging / "unpacked"
        unpacked.mkdir()
        extract(archive, unpacked)
        (unpacked / COMPLETE_MARKER).write_text(f"{asset_name}\n{expected_sha256}\n", encoding="utf-8")

        if tag_dir.exists():
            shutil.rmtree(tag_dir)
        shutil.move(str(unpacked), str(tag_dir))
    finally:
        shutil.rmtree(staging, ignore_errors=True)

    return tag_dir


def main() -> int:
    asset_name, expected_sha256 = platform_asset()
    url = f"{BASE_URL}/{asset_name}"
    tag_dir = install(TAG_DIR, asset_name, expected_sha256, url=url)
    shared = library(tag_dir)
    print(f"onnxruntime ready: {shared}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
