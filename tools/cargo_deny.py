#!/usr/bin/env python3
"""Install and run the pinned cargo-deny binary.

The gate runs `cargo deny check licenses`. cargo-deny is not a workspace
dependency. This script downloads one pinned prebuilt release into
`.cache/cargo-deny/<tag>/`, checks its SHA-256, and then runs it with the
arguments it is given:

    python tools/cargo_deny.py check licenses

With no arguments the script installs the binary and prints its path. The script
is idempotent. It checks the digest before it extracts, and it leaves no partial
folder when the check fails. Standard library only. It never writes outside
`.cache/cargo-deny`.

The pin and the digests come from the cargo-deny release page:

    https://github.com/EmbarkStudios/cargo-deny/releases/tag/0.20.2

cargo-deny publishes a Linux build only for the musl target. That static binary
runs on the glibc 2.35 baseline of Ubuntu 22.04 (SRS appendix).
"""

from __future__ import annotations

import hashlib
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
from pathlib import Path

# One pin. Confirm the current release at
# https://github.com/EmbarkStudios/cargo-deny/releases before you change it.
TAG = "0.20.2"
ASSETS = {
    "win": (
        "cargo-deny-0.20.2-x86_64-pc-windows-msvc.tar.gz",
        "975a22143262fd27476d19ee00c7af67978426e40e1dee94eed6bbade1cf87dc",
    ),
    "linux": (
        "cargo-deny-0.20.2-x86_64-unknown-linux-musl.tar.gz",
        "9f12ed4c49936e09b48bf862b595cde2fe64fcbd9d74dfacac6131ca824c8d5f",
    ),
}
BASE_URL = f"https://github.com/EmbarkStudios/cargo-deny/releases/download/{TAG}"
REPO_ROOT = Path(__file__).resolve().parents[1]
TAG_DIR = REPO_ROOT / ".cache" / "cargo-deny" / TAG
COMPLETE_MARKER = ".complete"
BINARY_NAME = "cargo-deny.exe" if sys.platform.startswith("win") else "cargo-deny"


class ChecksumError(RuntimeError):
    """The downloaded asset does not match the pinned SHA-256."""


def platform_asset() -> tuple[str, str]:
    """The (asset name, sha256) for the current platform."""
    if sys.platform.startswith("win"):
        return ASSETS["win"]
    if sys.platform.startswith("linux"):
        return ASSETS["linux"]
    raise SystemExit(f"no pinned cargo-deny asset for this platform: {sys.platform}")


def sha256_of(path: Path) -> str:
    """The hex SHA-256 of a file."""
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def binary(tag_dir: Path) -> Path | None:
    """The cargo-deny binary inside an extracted tag directory, if it is there."""
    for candidate in sorted(tag_dir.glob(f"*/{BINARY_NAME}")):
        if candidate.is_file():
            return candidate
    return None


def is_complete(tag_dir: Path) -> bool:
    """True when the tag directory holds a finished install."""
    return (tag_dir / COMPLETE_MARKER).is_file() and binary(tag_dir) is not None


def extract(archive: Path, dest: Path) -> None:
    """Extract a `.tar.gz` archive into `dest`."""
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


def ensure_binary() -> Path:
    """Install the pinned binary when it is missing, and return its path."""
    asset_name, expected_sha256 = platform_asset()
    url = f"{BASE_URL}/{asset_name}"
    tag_dir = install(TAG_DIR, asset_name, expected_sha256, url=url)
    found = binary(tag_dir)
    if found is None:
        raise SystemExit(f"the pinned cargo-deny archive holds no {BINARY_NAME}")
    return found


def main(argv: list[str]) -> int:
    executable = ensure_binary()
    if not argv:
        print(f"cargo-deny ready: {executable}")
        return 0
    # Run cargo-deny in the repository root so it finds `deny.toml`.
    result = subprocess.run([str(executable), *argv], cwd=REPO_ROOT)
    return result.returncode


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
