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


def test_the_pin_names_the_microsoft_nuget_package_for_both_platforms() -> None:
    module = load_module()
    assert re.fullmatch(r"v\d+\.\d+\.\d+", module.TAG), module.TAG
    assert set(module.ASSETS) == {"win-x64", "linux-x64"}
    version = module.TAG.lstrip("v")
    for name, digest in module.ASSETS.values():
        # The Microsoft-published package on nuget.org (task 62).
        assert name == f"microsoft.ml.onnxruntime.{version}.nupkg", name
        assert len(digest) == 64 and all(c in "0123456789abcdef" for c in digest), name
    assert module.BASE_URL.startswith("https://api.nuget.org/v3-flatcontainer/microsoft.ml.onnxruntime/")
    # The whole point of the pin: a release that matches the pinned ort API.
    assert version.startswith("1.28")


def nupkg(path: Path) -> str:
    """Write a NuGet-shaped package with two platforms and return its SHA-256."""
    import hashlib
    import zipfile

    with zipfile.ZipFile(path, "w") as handle:
        handle.writestr("LICENSE", "license text\n")
        handle.writestr("ThirdPartyNotices.txt", "notices text\n")
        handle.writestr("Microsoft.ML.OnnxRuntime.nuspec", "<package/>\n")
        handle.writestr("runtimes/win-x64/native/onnxruntime.dll", "win-x64 runtime")
        handle.writestr("runtimes/win-x64/native/onnxruntime_providers_shared.dll", "win-x64 shared")
        handle.writestr("runtimes/linux-x64/native/libonnxruntime.so", "linux-x64 runtime")
        handle.writestr("runtimes/win-arm64/native/onnxruntime.dll", "win-arm64 runtime")
    return hashlib.sha256(path.read_bytes()).hexdigest()


@pytest.mark.parametrize(
    ("rid", "library_name", "content"),
    [
        ("win-x64", "onnxruntime.dll", b"win-x64 runtime"),
        ("linux-x64", "libonnxruntime.so", b"linux-x64 runtime"),
    ],
)
def test_a_nuget_package_installs_one_platform_with_its_documents(
    tmp_path: Path, rid: str, library_name: str, content: bytes
) -> None:
    module = load_module()
    package = tmp_path / "package.nupkg"
    digest = nupkg(package)
    tag_dir = tmp_path / "onnxruntime" / module.TAG

    module.install(
        tag_dir, package.name, digest, url="stub", rid=rid, fetch=lambda url: package.read_bytes()
    )

    found = module.library(tag_dir)
    assert found is not None and found.name == library_name
    assert found.read_bytes() == content
    # The Rust tests and the notice generator read `<tag>/<folder>/lib/` and
    # `<tag>/<folder>/{LICENSE,ThirdPartyNotices.txt}`.
    folder = found.parent.parent
    assert found.parent.name == "lib" and folder.parent == tag_dir
    assert (folder / "LICENSE").read_text(encoding="utf-8") == "license text\n"
    assert (folder / "ThirdPartyNotices.txt").read_text(encoding="utf-8") == "notices text\n"
    # Only the requested platform's libraries are installed.
    other = {"win-x64": "libonnxruntime.so", "linux-x64": "onnxruntime.dll"}[rid]
    assert not (found.parent / other).exists()
    assert module.is_complete(tag_dir)


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
