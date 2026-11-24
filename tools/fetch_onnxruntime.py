#!/usr/bin/env python3
"""Download the pinned ONNX Runtime build into `.cache/onnxruntime/<tag>/`.

The inference engine loads ONNX Runtime at run time from a library path that the caller
gives (`melomae_core::inference::load_onnx_backend`). The engine never links the runtime
statically. The tests and development use the library this script fetches.

    python tools/fetch_onnxruntime.py

This script holds one pin: the Microsoft ONNX Runtime release whose C API matches the
pinned `ort` crate. `ort` 2.0.0-rc.13 uses ONNX Runtime 1.28 (ORT API version 27), so the
pin is the `1.28.0` release of that minor line. The pin was `v1.28.3` from the GitHub
release, until `github.com/microsoft/onnxruntime` began to return 404 (task 62). The 1.28
patch releases were published on GitHub only, so the pin moved to 1.28.0, the newest 1.28
build that Microsoft still publishes. The source is the `Microsoft.ML.OnnxRuntime` package
on nuget.org, which carries both platforms:

    win-x64    runtimes/win-x64/native/onnxruntime.dll
    linux-x64  runtimes/linux-x64/native/libonnxruntime.so

The script installs the current platform's libraries into `<tag>/<folder>/lib/` and the
package's `LICENSE` and `ThirdPartyNotices.txt` into `<tag>/<folder>/`. The Rust tests and
`tools/generate_notices.py` read that layout.

The Linux library keeps the glibc 2.35 baseline of SRS §2. A read of
`libonnxruntime.so` 1.28.0 shows its newest glibc requirement is `GLIBC_2.27`, and it holds
no `__isoc23_*` symbol (task 62). The glibc 2.35 of Ubuntu 22.04 is therefore sufficient.
The pyke.io binary that `ort`'s default features download is built with a newer toolchain,
and its `__isoc23_strtol`, `__isoc23_strtoll`, and `__isoc23_strtoull` need glibc 2.38.
That is why this script pins the Microsoft build.

It is idempotent: when the pinned build is already installed it does nothing. It checks
the SHA-256 before it extracts, and it leaves no partial folder when the check fails.
Standard library only. It never writes outside `.cache/onnxruntime`.
"""

from __future__ import annotations

import hashlib
import shutil
import sys
import tempfile
import urllib.request
import zipfile
from pathlib import Path

# One pin: a Microsoft ONNX Runtime release that matches the pinned `ort` API. One package
# serves both platforms. The SHA-256 is of the `.nupkg` file that nuget.org serves, measured
# on 2026-11-24 (task 62).
VERSION = "1.28.0"
TAG = f"v{VERSION}"
PACKAGE = f"microsoft.ml.onnxruntime.{VERSION}.nupkg"
PACKAGE_SHA256 = "769d1d3ea8ab6cd69f737c9dd4d4462aa4ad0ccfa106eaf506efc40d7bead5db"
ASSETS = {
    "win-x64": (PACKAGE, PACKAGE_SHA256),
    "linux-x64": (PACKAGE, PACKAGE_SHA256),
}
BASE_URL = f"https://api.nuget.org/v3-flatcontainer/microsoft.ml.onnxruntime/{VERSION}"
# The package's own documents, which the notice generator reads.
DOCUMENTS = ("LICENSE", "ThirdPartyNotices.txt")
REPO_ROOT = Path(__file__).resolve().parents[1]
TAG_DIR = REPO_ROOT / ".cache" / "onnxruntime" / TAG
COMPLETE_MARKER = ".complete"


class ChecksumError(RuntimeError):
    """The downloaded asset does not match the pinned SHA-256."""


def platform_rid() -> str:
    """The NuGet runtime identifier of the current platform."""
    if sys.platform.startswith("win"):
        return "win-x64"
    if sys.platform.startswith("linux"):
        return "linux-x64"
    raise SystemExit(f"no pinned ONNX Runtime asset for this platform: {sys.platform}")


def platform_asset() -> tuple[str, str]:
    """The (asset name, sha256) for the current platform."""
    return ASSETS[platform_rid()]


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


def extract(archive: Path, dest: Path, rid: str) -> None:
    """Install one platform of a NuGet package into `dest/onnxruntime-<rid>-<version>/`.

    The platform's libraries under `runtimes/<rid>/native/` go into `lib/`, and the
    package's documents go beside it. Every other member is left out.
    """
    folder = dest / f"onnxruntime-{rid}-{VERSION}"
    lib = folder / "lib"
    lib.mkdir(parents=True)
    native = f"runtimes/{rid}/native/"
    with zipfile.ZipFile(archive) as handle:
        for member in handle.infolist():
            name = member.filename
            if name.startswith(native) and not member.is_dir() and "/" not in name[len(native):]:
                target = lib / name[len(native):]
            elif name in DOCUMENTS:
                target = folder / name
            else:
                continue
            with handle.open(member) as source:
                target.write_bytes(source.read())


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
    rid: str | None = None,
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
        extract(archive, unpacked, rid or platform_rid())
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
