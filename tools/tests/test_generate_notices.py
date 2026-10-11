"""Tests for the third-party notices generator.

The unit tests cover the license-file mapping and the staleness check. The end
to end test runs the tool on the real repository.
"""

from __future__ import annotations

import importlib.util
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = Path(__file__).resolve().parents[1] / "generate_notices.py"
OUTPUT = REPO_ROOT / "THIRD_PARTY_NOTICES.md"


def load_module():
    spec = importlib.util.spec_from_file_location("generate_notices", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def test_normalized_flattens_a_license_file_name() -> None:
    module = load_module()
    assert module.normalized("LICENSE-MIT") == "LICENSE-MIT"
    assert module.normalized("LICENSE.MIT") == "LICENSE-MIT"
    assert module.normalized("LICENSE-ZLIB.md") == "LICENSE-ZLIB"
    assert module.normalized("LICENCE") == "LICENCE"


def test_sole_id_keeps_a_with_clause_and_rejects_a_combination() -> None:
    module = load_module()
    assert module.sole_id("MIT") == "MIT"
    assert module.sole_id("Apache-2.0 WITH LLVM-exception") == "Apache-2.0 WITH LLVM-exception"
    assert module.sole_id("MIT OR Apache-2.0") is None
    assert module.sole_id("MIT/Apache-2.0") is None
    assert module.sole_id("(MIT OR Apache-2.0) AND Unicode-3.0") is None


def test_named_ids_reads_a_deprecated_and_a_conjunction_expression() -> None:
    module = load_module()
    assert module.named_ids("(MIT OR Apache-2.0) AND Unicode-3.0") == {
        "MIT",
        "Apache-2.0",
        "Unicode-3.0",
    }
    assert module.named_ids("MIT/Apache-2.0") == {"MIT", "Apache-2.0"}
    assert module.named_ids("Apache-2.0 WITH LLVM-exception OR MIT") == {
        "Apache-2.0 WITH LLVM-exception",
        "MIT",
    }


def test_id_for_filename_maps_the_known_spellings() -> None:
    module = load_module()
    assert module.id_for_filename("LICENSE-MIT", "MIT OR Apache-2.0") == "MIT"
    assert module.id_for_filename("LICENSE-APACHE", "MIT OR Apache-2.0") == "Apache-2.0"
    assert module.id_for_filename("LICENSE-0BSD", "0BSD OR MIT") == "0BSD"
    assert module.id_for_filename("LICENSE-ZLIB.md", "Zlib") == "Zlib"
    assert module.id_for_filename("UNLICENSE", "Unlicense OR MIT") == "Unlicense"
    assert module.id_for_filename("LICENSE-UNICODE", "Unicode-3.0") == "Unicode-3.0"
    assert module.id_for_filename("LICENSE.BSD-3-Clause", "BSD-3-Clause AND MIT") == "BSD-3-Clause"


def test_id_for_filename_uses_a_lone_expression_for_a_generic_name() -> None:
    module = load_module()
    assert module.id_for_filename("LICENSE", "ISC") == "ISC"
    assert module.id_for_filename("LICENSE", "Apache-2.0 WITH LLVM-exception") == (
        "Apache-2.0 WITH LLVM-exception"
    )
    # A generic name cannot stand for one branch of a choice.
    assert module.id_for_filename("LICENSE", "CC0-1.0 OR MIT-0 OR Apache-2.0") is None


def test_clean_text_normalizes_line_endings_and_strips_blank_edges() -> None:
    module = load_module()
    assert module.clean_text("a\r\nb\r\n\r\n") == "a\nb"
    assert module.clean_text("\n\nx\n") == "x"


def test_unique_components_sorts_and_drops_a_duplicate() -> None:
    module = load_module()
    components = [
        {"name": "b", "version": "1"},
        {"name": "A", "version": "2"},
        {"name": "b", "version": "1"},
    ]
    result = module.unique_components(components)
    assert [item["name"] for item in result] == ["A", "b"]
    assert len(result) == 2


def test_the_check_mode_reports_a_stale_file(tmp_path, monkeypatch) -> None:
    module = load_module()
    target = tmp_path / "THIRD_PARTY_NOTICES.md"
    target.write_text("old\n", encoding="utf-8")
    monkeypatch.setattr(module, "OUTPUT", target)
    monkeypatch.setattr(module, "build_notices", lambda: "new\n")

    assert module.main(["--check"]) == 1
    target.write_text("new\n", encoding="utf-8")
    assert module.main(["--check"]) == 0


def test_the_committed_notice_names_the_bundled_components() -> None:
    text = OUTPUT.read_text(encoding="utf-8")
    for expected in (
        "| onnxruntime | 1.28.3 | MIT |",
        "| Apollo model (apollo.onnx) | c68bd80f | CC-BY-SA-4.0 |",
        "| symphonia | 0.6.1 | MPL-2.0 |",
        "| opusic-c | 1.6.1 | BSD-3-Clause |",
        "| @tauri-apps/api | 2.11.1 | Apache-2.0 OR MIT |",
    ):
        assert expected in text, expected


def test_e2e_the_committed_notice_matches_the_dependency_trees() -> None:
    result = subprocess.run(
        [sys.executable, str(SCRIPT), "--check"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    assert result.returncode == 0, result.stderr
