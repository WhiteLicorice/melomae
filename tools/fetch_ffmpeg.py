#!/usr/bin/env python3
"""Download the pinned ffmpeg build into `.cache/ffmpeg/<tag>/`.

The decode tests use ffmpeg to build their inputs and their reference decodes. ffmpeg is
development and CI only: the decoder never runs it. This script holds one pin, a BtbN
monthly-archive release, and installs the asset for the current operating system.

    python tools/fetch_ffmpeg.py

It is idempotent: when the pinned build is already installed it does nothing. It checks the
SHA-256 before it extracts, and it leaves no partial folder when the check fails. Standard
library only. It never writes outside `.cache/ffmpeg`.
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

# One pin: a BtbN monthly-archive release. BtbN prunes daily autobuilds, so a monthly archive is
# the stable choice.
TAG = "autobuild-2026-08-31-13-27"
ASSETS = {
    "win64-gpl": (
        "ffmpeg-N-126342-gf88b741dbf-win64-gpl.zip",
        "b4da332540eaebc6939181b59e267f163dd57407ef6596f7f3452845921d1d91",
    ),
    "linux64-gpl": (
        "ffmpeg-N-126342-gf88b741dbf-linux64-gpl.tar.xz",
        "d1cf19f669510448f18a4cffcdbd8fa9592ee7c15c92feb5b96ad7e9ccc30114",
    ),
}
BASE_URL = f"https://github.com/BtbN/FFmpeg-Builds/releases/download/{TAG}"
REPO_ROOT = Path(__file__).resolve().parents[1]
TAG_DIR = REPO_ROOT / ".cache" / "ffmpeg" / TAG
COMPLETE_MARKER = ".complete"


class ChecksumError(RuntimeError):
    """The downloaded asset does not match the pinned SHA-256."""


def platform_asset() -> tuple[str, str]:
    """The (asset name, sha256) for the current platform."""
    if sys.platform.startswith("win"):
        return ASSETS["win64-gpl"]
    if sys.platform.startswith("linux"):
        return ASSETS["linux64-gpl"]
    raise SystemExit(f"no pinned ffmpeg asset for this platform: {sys.platform}")


def sha256_of(path: Path) -> str:
    """The hex SHA-256 of a file."""
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def ffmpeg_binary(tag_dir: Path) -> Path | None:
    """The ffmpeg binary inside an extracted tag directory, if it is there."""
    for candidate in sorted(tag_dir.glob("*/bin/ffmpeg*")):
        if candidate.is_file() and candidate.suffix in ("", ".exe"):
            return candidate
    return None


def is_complete(tag_dir: Path) -> bool:
    """True when the tag directory holds a finished install."""
    return (tag_dir / COMPLETE_MARKER).is_file() and ffmpeg_binary(tag_dir) is not None


def extract(archive: Path, dest: Path) -> None:
    """Extract a `.zip` or `.tar.xz` archive into `dest`."""
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
    binary = ffmpeg_binary(tag_dir)
    print(f"ffmpeg ready: {binary}")
    print(f"put {binary.parent} on PATH when you run the tests outside pnpm verify")
    return 0


if __name__ == "__main__":
    sys.exit(main())
