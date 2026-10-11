"""Tests for the pinned-ONNX-Runtime fetch script.

These tests never touch the network: the download is replaced with a stub.
"""

from __future__ import annotations

import importlib.util
import re
import sys
from pathlib import Path

import pytest

SCRIPT = Path(__file__).resolve().parents[1] / "fetch_onnxruntime.py"

# The shared library name inside an extracted release, for the current platform.
LIBRARY = "onnxruntime.dll" if sys.platform.startswith("win") else "libonnxruntime.so"


def load_module():
    spec = importlib.util.spec_from_file_location("fetch_onnxruntime", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def test_the_pin_names_a_microsoft_release_and_both_assets() -> None:
    module = load_module()
    # A GitHub release tag from microsoft/onnxruntime: "v" and a semantic version.
    assert re.fullmatch(r"v\d+\.\d+\.\d+", module.TAG), module.TAG
    assert set(module.ASSETS) == {"win-x64", "linux-x64"}
    for name, digest in module.ASSETS.values():
        assert len(digest) == 64 and all(c in "0123456789abcdef" for c in digest), name
    # The whole point of the pin: a release that matches the pinned ort API.
    assert module.TAG.lstrip("v").startswith("1.28")


def test_a_checksum_mismatch_is_rejected_and_leaves_no_partial_folder(tmp_path: Path) -> None:
    module = load_module()
    tag_dir = tmp_path / "onnxruntime" / module.TAG

    with pytest.raises(module.ChecksumError):
        module.install(tag_dir, "asset.zip", "0" * 64, url="stub", fetch=lambda url: b"payload")

    assert not tag_dir.exists(), "a failed install must leave no tag folder"
    assert list(tag_dir.parent.iterdir()) == [], "a failed install must leave no staging folder"


def test_a_complete_folder_is_not_downloaded_again(tmp_path: Path) -> None:
    module = load_module()
    tag_dir = tmp_path / "onnxruntime" / module.TAG
    binary = tag_dir / "onnxruntime-pinned" / "lib" / LIBRARY
    binary.parent.mkdir(parents=True)
    binary.write_bytes(b"binary")
    (tag_dir / module.COMPLETE_MARKER).write_text("done\n", encoding="utf-8")

    def explode(url: str) -> bytes:
        raise AssertionError("a complete install must not download again")

    result = module.install(tag_dir, "asset.zip", "0" * 64, url="stub", fetch=explode)

    assert result == tag_dir
    assert binary.read_bytes() == b"binary"


def test_a_checksum_mismatch_after_a_previous_install_keeps_the_old_folder(tmp_path: Path) -> None:
    module = load_module()
    tag_dir = tmp_path / "onnxruntime" / module.TAG
    tag_dir.mkdir(parents=True)
    (tag_dir / "keep.txt").write_text("old\n", encoding="utf-8")

    with pytest.raises(module.ChecksumError):
        module.install(tag_dir, "asset.zip", "f" * 64, url="stub", fetch=lambda url: b"payload")

    assert (tag_dir / "keep.txt").read_text(encoding="utf-8") == "old\n"


def test_library_finds_the_extracted_library(tmp_path: Path) -> None:
    module = load_module()
    tag_dir = tmp_path / module.TAG
    binary = tag_dir / "onnxruntime-pinned" / "lib" / LIBRARY
    binary.parent.mkdir(parents=True)
    binary.write_bytes(b"binary")

    assert module.library(tag_dir) == binary
    assert module.library(tmp_path / "missing") is None
