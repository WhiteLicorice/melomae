"""Tests for the pinned-cargo-deny fetch-and-run script.

These tests never touch the network: the download is replaced with a stub.
"""

from __future__ import annotations

import importlib.util
import re
from pathlib import Path

import pytest

SCRIPT = Path(__file__).resolve().parents[1] / "cargo_deny.py"


def load_module():
    spec = importlib.util.spec_from_file_location("cargo_deny", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def test_the_pin_names_a_release_and_both_assets() -> None:
    module = load_module()
    # A GitHub release tag from EmbarkStudios/cargo-deny: a semantic version.
    assert re.fullmatch(r"\d+\.\d+\.\d+", module.TAG), module.TAG
    assert set(module.ASSETS) == {"win", "linux"}
    for name, digest in module.ASSETS.values():
        assert len(digest) == 64 and all(c in "0123456789abcdef" for c in digest), name


def test_a_checksum_mismatch_is_rejected_and_leaves_no_partial_folder(tmp_path: Path) -> None:
    module = load_module()
    tag_dir = tmp_path / "cargo-deny" / module.TAG

    with pytest.raises(module.ChecksumError):
        module.install(tag_dir, "asset.tar.gz", "0" * 64, url="stub", fetch=lambda url: b"payload")

    assert not tag_dir.exists(), "a failed install must leave no tag folder"
    assert list(tag_dir.parent.iterdir()) == [], "a failed install must leave no staging folder"


def test_a_complete_folder_is_not_downloaded_again(tmp_path: Path) -> None:
    module = load_module()
    tag_dir = tmp_path / "cargo-deny" / module.TAG
    executable = tag_dir / "cargo-deny-pinned" / module.BINARY_NAME
    executable.parent.mkdir(parents=True)
    executable.write_bytes(b"binary")
    (tag_dir / module.COMPLETE_MARKER).write_text("done\n", encoding="utf-8")

    def explode(url: str) -> bytes:
        raise AssertionError("a complete install must not download again")

    result = module.install(tag_dir, "asset.tar.gz", "0" * 64, url="stub", fetch=explode)

    assert result == tag_dir
    assert executable.read_bytes() == b"binary"


def test_a_checksum_mismatch_after_a_previous_install_keeps_the_old_folder(tmp_path: Path) -> None:
    module = load_module()
    tag_dir = tmp_path / "cargo-deny" / module.TAG
    tag_dir.mkdir(parents=True)
    (tag_dir / "keep.txt").write_text("old\n", encoding="utf-8")

    with pytest.raises(module.ChecksumError):
        module.install(tag_dir, "asset.tar.gz", "f" * 64, url="stub", fetch=lambda url: b"payload")

    assert (tag_dir / "keep.txt").read_text(encoding="utf-8") == "old\n"


def test_binary_finds_the_extracted_executable(tmp_path: Path) -> None:
    module = load_module()
    tag_dir = tmp_path / module.TAG
    executable = tag_dir / "cargo-deny-pinned" / module.BINARY_NAME
    executable.parent.mkdir(parents=True)
    executable.write_bytes(b"binary")

    assert module.binary(tag_dir) == executable
    assert module.binary(tmp_path / "missing") is None


def test_the_linux_asset_is_a_static_musl_build() -> None:
    module = load_module()
    # cargo-deny publishes no glibc Linux build. The musl build runs on the
    # Ubuntu 22.04 baseline.
    assert "musl" in module.ASSETS["linux"][0]
