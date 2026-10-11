"""The committed parity files match their SHA-256 sidecars.

This test uses the standard library only. It runs in CI, where the export group
is not installed, so it must not import torch, numpy, or soundfile.
"""

import hashlib
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

# The three committed large files, each with its SHA-256 in a text file beside
# it. These are the only exceptions to the "no audio, no model, no file over
# 1 MB" rule.
COMMITTED = (
    REPO_ROOT / "tools" / "parity" / "input_wav.wav",
    REPO_ROOT / "tools" / "parity" / "golden.wav",
    REPO_ROOT / "models" / "apollo.onnx",
)

LFS_POINTER = b"version https://git-lfs.github.com/spec/v1"


def test_each_committed_parity_file_matches_its_sha256():
    for path in COMMITTED:
        assert path.is_file(), f"{path} is missing. Run: git lfs pull"
        data = path.read_bytes()
        assert not data.startswith(LFS_POINTER), (
            f"{path} is a Git LFS pointer, not the data. Run: git lfs pull"
        )
        sidecar = Path(str(path) + ".sha256")
        assert sidecar.is_file(), f"{sidecar} is missing"
        expected = sidecar.read_text(encoding="utf-8").strip().lower()
        actual = hashlib.sha256(data).hexdigest()
        assert actual == expected, (
            f"{path} does not match {sidecar}: {actual} != {expected}"
        )
