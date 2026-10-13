"""Tests for the third-party notices generator.

The unit tests cover the license-file mapping and the staleness check. The end
to end test runs the tool on the real repository.
"""

from __future__ import annotations

import importlib.util
import subprocess
import sys
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = Path(__file__).resolve().parents[1] / "generate_notices.py"
OUTPUT = REPO_ROOT / "THIRD_PARTY_NOTICES.md"


def load_module():
    spec = importlib.util.spec_from_file_location("generate_notices", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def make_component(directory: Path, name: str, version: str, license: str, files: dict) -> dict:
    """A component source directory with the given license files."""
    directory.mkdir(parents=True)
    for filename, text in files.items():
        (directory / filename).write_text(text, encoding="utf-8")
    return {"name": name, "version": version, "license": license, "directory": directory}


def make_runtime(directory: Path, with_third_party: bool = True) -> Path:
    """A fixture that stands in for the extracted ONNX Runtime archive."""
    inner = directory / "onnxruntime-fake-1.28.3"
    inner.mkdir(parents=True)
    (inner / "LICENSE").write_text("Fake runtime license text.\n", encoding="utf-8")
    if with_third_party:
        (inner / "ThirdPartyNotices.txt").write_text(
            "Fake third party notices for the runtime.\n", encoding="utf-8"
        )
    return directory


def patch_sources(monkeypatch, module, components: list, runtime_dir: Path) -> None:
    """Feed the generator a fixed component list and runtime folder."""
    monkeypatch.setattr(module, "cargo_metadata", lambda: {})
    monkeypatch.setattr(module, "rust_components", lambda metadata: components)
    monkeypatch.setattr(module, "pnpm_components", lambda: [])
    monkeypatch.setattr(module, "RUNTIME_DIR", runtime_dir, raising=False)


def section_with(output: str, needle: str) -> str:
    """The `### ...` section of the generated notice that holds a line."""
    for section in output.split("\n### "):
        if needle in section:
            return section
    raise AssertionError(f"no section holds {needle!r}")


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


MIT_BODY = """MIT License

Copyright (c) {year} {holder}

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND.
"""

BSD_BODY = """BSD 3-Clause License

Copyright (c) {year}, {holder}
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice.
2. Redistributions in binary form must reproduce the above copyright notice.
3. Neither the name of the copyright holder may be used to endorse products.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS".
"""


def test_every_component_keeps_its_own_license_text(tmp_path, monkeypatch) -> None:
    module = load_module()
    components = [
        make_component(
            tmp_path / "alpha",
            "alpha",
            "1.0.0",
            "MIT",
            {"LICENSE": MIT_BODY.format(year="2020", holder="Alpha Authors")},
        ),
        make_component(
            tmp_path / "beta",
            "beta",
            "2.0.0",
            "MIT",
            {"LICENSE": MIT_BODY.format(year="2021", holder="Beta Authors")},
        ),
        make_component(
            tmp_path / "gamma",
            "gamma",
            "3.0.0",
            "BSD-3-Clause",
            {"LICENSE": BSD_BODY.format(year="2019", holder="Gamma Authors")},
        ),
        make_component(
            tmp_path / "delta",
            "delta",
            "4.0.0",
            "BSD-3-Clause",
            {"LICENSE": BSD_BODY.format(year="2018", holder="Delta Authors")},
        ),
    ]
    patch_sources(monkeypatch, module, components, make_runtime(tmp_path / "runtime"))

    output = module.build_notices()

    expected = [
        ("alpha", "1.0.0", "Copyright (c) 2020 Alpha Authors"),
        ("beta", "2.0.0", "Copyright (c) 2021 Beta Authors"),
        ("gamma", "3.0.0", "Copyright (c) 2019, Gamma Authors"),
        ("delta", "4.0.0", "Copyright (c) 2018, Delta Authors"),
    ]
    for name, version, copyright_line in expected:
        assert copyright_line in output, copyright_line
        assert f"{name} {version}" in section_with(output, copyright_line), name


def test_a_notice_file_is_kept_in_full(tmp_path, monkeypatch) -> None:
    module = load_module()
    notice = "Beta Project Notice\n\nThis product includes software developed by the Beta Project."
    components = [
        make_component(
            tmp_path / "epsilon",
            "epsilon",
            "1.0.0",
            "Apache-2.0",
            {
                "LICENSE": "Apache License\nVersion 2.0, January 2004\nhttp://www.apache.org/licenses/",
                "NOTICE": notice,
            },
        ),
    ]
    patch_sources(monkeypatch, module, components, make_runtime(tmp_path / "runtime"))

    output = module.build_notices()

    assert "Apache License" in output
    assert notice in output
    assert "epsilon 1.0.0" in section_with(output, "developed by the Beta Project")


def test_the_bundled_runtime_documents_are_kept_and_required(tmp_path, monkeypatch) -> None:
    module = load_module()
    patch_sources(monkeypatch, module, [], make_runtime(tmp_path / "runtime"))

    output = module.build_notices()

    assert "Fake runtime license text." in output
    assert "Fake third party notices for the runtime." in output
    assert "onnxruntime 1.28.3" in section_with(output, "Fake third party notices")


def test_a_missing_runtime_document_fails_the_generator(tmp_path, monkeypatch) -> None:
    module = load_module()
    patch_sources(
        monkeypatch,
        module,
        [],
        make_runtime(tmp_path / "runtime", with_third_party=False),
    )

    with pytest.raises(SystemExit):
        module.build_notices()


def test_the_committed_notice_keeps_the_component_copyrights() -> None:
    text = OUTPUT.read_text(encoding="utf-8")
    for expected in (
        "Meta Platforms",
        "Douman",
        "Xiph.Org",
        "Microsoft",
        "protocolbuffers/protobuf",
        "gtk-rs Project",
        "1991-2018 Unicode",
    ):
        assert expected in text, expected


def make_nested_file(directory: Path, relative: str, text: str) -> Path:
    """A license document in a component subdirectory."""
    path = directory / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    return path


def test_a_separate_copyright_file_is_kept_with_its_component(tmp_path, monkeypatch) -> None:
    module = load_module()
    license_text = MIT_BODY.format(year="2022", holder="Copy Right Authors")
    copyright_text = "Copyright (c) 2022 Copy Right Authors. All rights reserved."
    components = [
        make_component(
            tmp_path / "zeta",
            "zeta",
            "1.0.0",
            "MIT",
            {"LICENSE": license_text, "COPYRIGHT": copyright_text},
        ),
    ]
    patch_sources(monkeypatch, module, components, make_runtime(tmp_path / "runtime"))

    output = module.build_notices()

    assert license_text in output
    assert copyright_text in output
    assert "zeta 1.0.0" in section_with(output, copyright_text)


def test_a_nested_unicode_table_license_is_kept_with_its_component(tmp_path, monkeypatch) -> None:
    module = load_module()
    unicode_text = (
        "UNICODE, INC. LICENSE AGREEMENT - DATA FILES AND SOFTWARE\n\n"
        "Copyright \u00a9 1991-2018 Unicode, Inc. All rights reserved."
    )
    directory = tmp_path / "regexlike"
    make_component(
        directory,
        "regexlike",
        "0.8.11",
        "MIT OR Apache-2.0",
        {"LICENSE-MIT": MIT_BODY.format(year="2014", holder="Rust Project Developers")},
    )
    make_nested_file(directory, "src/unicode_tables/LICENSE-UNICODE", unicode_text)
    components = [
        {
            "name": "regexlike",
            "version": "0.8.11",
            "license": "MIT OR Apache-2.0",
            "directory": directory,
        },
    ]
    patch_sources(monkeypatch, module, components, make_runtime(tmp_path / "runtime"))

    output = module.build_notices()

    assert unicode_text in output
    section = section_with(output, "1991-2018 Unicode")
    assert "regexlike 0.8.11" in section
    assert "src/unicode_tables/LICENSE-UNICODE" in section


def test_a_copyright_only_component_is_not_called_license_less(tmp_path, monkeypatch) -> None:
    module = load_module()
    rust_text = (
        "The Rust Project is dual-licensed under Apache 2.0 and MIT terms.\n"
        "Copyright 2014 The Rust Project Developers."
    )
    components = [
        make_component(
            tmp_path / "cesu",
            "cesu",
            "1.1.0",
            "MIT OR Apache-2.0",
            {"COPYRIGHT-RUST.txt": rust_text},
        ),
    ]
    patch_sources(monkeypatch, module, components, make_runtime(tmp_path / "runtime"))

    output = module.build_notices()

    assert rust_text in output
    assert "cesu 1.1.0" in section_with(output, "The Rust Project is dual-licensed")
    assert "ship no license file" not in output


def test_a_nested_license_is_kept_but_test_and_bench_folders_are_excluded(tmp_path, monkeypatch) -> None:
    module = load_module()
    components = [
        make_component(
            tmp_path / "packed",
            "packed",
            "1.0.0",
            "MIT",
            {"LICENSE": MIT_BODY.format(year="2020", holder="Packed Authors")},
        ),
    ]
    directory = components[0]["directory"]
    make_nested_file(directory, "src/vendored/COPYRIGHT", "VENDORED COMMON COPYRIGHT\n")
    make_nested_file(directory, "tests/LICENSE-FIXTURE", "FIXTURE ONLY NOTICE\n")
    make_nested_file(directory, "benches/COPYING", "BENCH ONLY COPYING\n")
    make_nested_file(directory, "examples/LICENSE", "EXAMPLE ONLY LICENSE\n")
    patch_sources(monkeypatch, module, components, make_runtime(tmp_path / "runtime"))

    output = module.build_notices()

    assert "Packed Authors" in output
    assert "VENDORED COMMON COPYRIGHT" in output
    assert "packed 1.0.0" in section_with(output, "VENDORED COMMON COPYRIGHT")
    assert "src/vendored/COPYRIGHT" in section_with(output, "VENDORED COMMON COPYRIGHT")
    assert "FIXTURE ONLY NOTICE" not in output
    assert "BENCH ONLY COPYING" not in output
    assert "EXAMPLE ONLY LICENSE" not in output


def test_the_generated_app_declaration_offers_version_3_only() -> None:
    module = load_module()
    text = module.build_notices(rust=[], scripts=[], runtime=[])
    assert "Melomae is licensed under the GNU General Public License, version 3 only" in text
    # The old wording, split so this guard does not carry the phrase itself.
    assert "version 3 " + "or " + "later" not in text
