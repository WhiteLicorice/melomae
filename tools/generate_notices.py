#!/usr/bin/env python3
"""Generate `THIRD_PARTY_NOTICES.md` from the dependency trees.

Melomae must ship a notice for every bundled component (SRS section 11). This
script builds that notice from four sources:

    - the Rust crates that the shipped build links (Cargo), normal and build
      dependencies for every target, without dev-dependencies;
    - the production pnpm dependencies (JavaScript);
    - the native libraries that a Rust crate vendors and links;
    - the components that no package manifest lists: the ONNX Runtime library
      and the Apollo model (Stack section J).

Every component keeps its own license, notice, and copyright texts, read from
the source that it ships. The texts are grouped only when they are byte-identical
after line-ending normalization, and each group names every component that it
covers. `NOTICE` and `COPYRIGHT` files are kept in full. A document beside the
code it covers, such as `src/unicode_tables/LICENSE-UNICODE`, is read too, and
the file records its path. A component that ships no such document is listed
under the standard SPDX text for one of its declared identifiers, and the file
records that it shipped none.

    python tools/generate_notices.py
    python tools/generate_notices.py --check

`--check` writes nothing. It fails when the committed file differs, so a change
to the dependency trees must regenerate the notice. The gate runs that check.

The output is deterministic: it holds no path, no date, and no host name. The
two lockfiles fix the component set. The standard SPDX texts are committed
under `tools/spdx_texts`. The bundled ONNX Runtime documents are read from the
pinned archive, whose Windows and Linux documents are identical. Standard
library only.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
OUTPUT = REPO_ROOT / "THIRD_PARTY_NOTICES.md"
SPDX_TEXT_DIR = Path(__file__).resolve().parent / "spdx_texts"

# The bundled ONNX Runtime release. `tools/fetch_onnxruntime.py` fetches this
# archive; the generator reads its own `LICENSE` and `ThirdPartyNotices.txt`
# from the extracted folder and fails when either is missing.
RUNTIME_VERSION = "1.28.3"
RUNTIME_TAG = f"v{RUNTIME_VERSION}"
RUNTIME_DIR = REPO_ROOT / ".cache" / "onnxruntime" / RUNTIME_TAG

# The components that no package manifest lists but the installer bundles
# (Stack section J). The version of the model is the checkpoint revision.
BUNDLED = [
    {
        "name": "onnxruntime",
        "version": RUNTIME_VERSION,
        "license": "MIT",
        "note": "The shared library the installer bundles (Stack section J).",
    },
    {
        "name": "Apollo model (apollo.onnx)",
        "version": "c68bd80f",
        "license": "CC-BY-SA-4.0",
        "note": "The restoration model. See models/NOTICE.md.",
    },
]

# Native libraries that a crate vendors and links. The crate package does not
# ship the library's license at its top level, so the generator reads it from
# the vendored source directory inside the crate. Keyed by crate name; each
# entry is `(relative path, SPDX id or None)`.
VENDORED_NATIVE = {
    "opusic-sys": [
        ("opus/COPYING", "BSD-3-Clause"),
        ("opus/AUTHORS", None),
    ],
}

# SQLite is in the public domain. The amalgamation that `libsqlite3-sys` bundles
# carries a blessing in place of a legal notice, which is reproduced here rather
# than the whole C file.
SQLITE_BLESSING = """SQLite is in the public domain.

The bundled SQLite amalgamation (`libsqlite3-sys`, `sqlite3/sqlite3.c`, SQLite
version 3.53.2) carries this blessing in place of a legal notice:

    The author disclaims copyright to this source code. In place of a legal
    notice, here is a blessing:

        May you do good and not evil.
        May you find forgiveness for yourself and forgive others.
        May you share freely, never taking more than you give.

Source: the bundled amalgamation and https://www.sqlite.org/copyright.html
"""

# The license text for an identifier that has no file in the dependency trees.
LICENSE_URLS = {
    "CC-BY-SA-4.0": "https://creativecommons.org/licenses/by-sa/4.0/legalcode.en",
    "CC0-1.0": "https://creativecommons.org/publicdomain/zero/1.0/legalcode.en",
    "MIT-0": "https://spdx.org/licenses/MIT-0.html",
    "LGPL-2.1-or-later": "https://www.gnu.org/licenses/old-licenses/lgpl-2.1.html",
}

# The order in which one identifier is chosen for a component that ships no
# license file. The permissive licenses come first, so a dual-licensed crate
# reads its standard text from the alternative that Melomae distributes under.
FALLBACK_PREFERENCE = [
    "MIT",
    "Apache-2.0",
    "BSD-3-Clause",
    "ISC",
    "Zlib",
    "Unicode-3.0",
    "0BSD",
    "CC0-1.0",
    "MIT-0",
    "Unlicense",
    "MPL-2.0",
]

LICENSE_FILE = re.compile(r"(?i)^(licen[cs]e|copying|copyright|unlicense|notice)")
# The nested walk matches a document name only: a license word, an optional
# `-suffix`, and an optional text extension. A source file such as
# `src/copying.rs` is not a license document and does not match.
NESTED_LICENSE_FILE = re.compile(
    r"(?i)^(licen[cs]e|copying|copyright|notice|unlicense)([-_][^.]*)?(\.(txt|md|rst))?$"
)
# Folders whose notice files describe test, benchmark, or example inputs, not
# the shipped code. The nested walk skips them.
EXCLUDED_DIRS = frozenset(
    {"test", "tests", "bench", "benches", "example", "examples", "fixture", "fixtures"}
)
# Nested notice documents that the shipped build does not compile, keyed by
# crate name and the document path inside the crate. Each entry records the
# evidence that its code is outside the shipped targets; the enabled feature
# set is read from `cargo metadata`.
EXCLUDED_NESTED = {
    # `vendored` is off (enabled: `default`, `pkg-config`), so the crate links
    # the system libdbus and never builds the bundled tree.
    "libdbus-sys": frozenset(
        {"vendor/dbus/COPYING", "vendor/dbus/cmake/modules/COPYING-CMAKE-SCRIPTS"}
    ),
    # `bundled-sqlcipher` is off (enabled: `bundled`), so the shipped build uses
    # the plain SQLite amalgamation, not the SQLCipher code.
    "libsqlite3-sys": frozenset({"sqlcipher/LICENSE"}),
    # The `spin` module is `#[cfg(not(feature = "std"))]`; `std` is enabled, so
    # the crate does not compile this file.
    "tracing-core": frozenset({"src/spin/LICENSE"}),
}
PREFIXES = ("LICENSE-", "LICENCE-", "COPYING-", "LICENSE", "LICENCE", "COPYING")
EXPRESSION_TOKENS = re.compile(r"\b(AND|OR)\b|[()/]")


def run(command: list[str], cwd: Path) -> str:
    """Run a command and return its standard output.

    On Windows the shell runs the command, so that a `.cmd` shim such as `pnpm`
    starts.
    """
    result = subprocess.run(
        command,
        cwd=cwd,
        capture_output=True,
        text=True,
        encoding="utf-8",
        shell=sys.platform.startswith("win"),
    )
    if result.returncode != 0:
        sys.stderr.write(result.stderr)
        raise SystemExit(f"command failed ({result.returncode}): {' '.join(command)}")
    return result.stdout


def cargo_metadata() -> dict:
    """The cargo metadata for the workspace, with the shipped dependency graph."""
    text = run(
        ["cargo", "metadata", "--format-version", "1", "--locked"],
        REPO_ROOT,
    )
    return json.loads(text)


def shipped_package_ids(metadata: dict) -> set[str]:
    """The ids of the third-party crates the shipped build links.

    The walk starts at the workspace crates and follows every dependency edge
    that is not a dev edge. It covers every target, so a Windows-only or
    Linux-only crate stays in the set.
    """
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    workspace = set(metadata["workspace_members"])
    seen: set[str] = set()
    stack = list(workspace)
    while stack:
        current = stack.pop()
        if current in seen:
            continue
        seen.add(current)
        for dependency in nodes[current]["deps"]:
            kinds = dependency.get("dep_kinds") or [{}]
            if any(kind.get("kind") != "dev" for kind in kinds):
                stack.append(dependency["pkg"])
    return seen - workspace


def rust_components(metadata: dict) -> list[dict]:
    """The shipped Rust components, each with its license source directory."""
    shipped = shipped_package_ids(metadata)
    components = []
    for package in metadata["packages"]:
        if package["id"] not in shipped:
            continue
        components.append(
            {
                "name": package["name"],
                "version": package["version"],
                "license": package.get("license") or "UNKNOWN",
                "directory": Path(package["manifest_path"]).parent,
            }
        )
    return components


def pnpm_components() -> list[dict]:
    """The production pnpm components, each with its license source directory."""
    text = run(["pnpm", "licenses", "list", "--prod", "--json"], REPO_ROOT)
    listing = json.loads(text)
    components = []
    for group in listing.values():
        for package in group:
            for version, path in zip(package["versions"], package["paths"]):
                components.append(
                    {
                        "name": package["name"],
                        "version": version,
                        "license": package["license"],
                        "directory": Path(path),
                    }
                )
    return components


def normalized(name: str) -> str:
    """A license file name with one spelling: upper case, no text extension.

    Only the usual text extensions come off. A name such as `LICENSE.MIT` keeps
    its `MIT` token.
    """
    stem = name
    for extension in (".md", ".txt", ".rst"):
        if stem.lower().endswith(extension):
            stem = stem[: -len(extension)]
            break
    return stem.upper().replace(".", "-").replace("_", "-")


def sole_id(expression: str) -> str | None:
    """The license id when the expression names exactly one id, else None."""
    if EXPRESSION_TOKENS.search(expression):
        return None
    return expression.strip() or None


def named_ids(expression: str) -> set[str]:
    """The SPDX ids that a license expression names.

    A `X WITH Y` clause stays one id. This covers the deprecated slash form,
    such as `MIT/Apache-2.0`.
    """
    found = set()
    for match in re.finditer(r"([A-Za-z0-9.\-]+)\s+WITH\s+([A-Za-z0-9.\-]+)", expression):
        found.add(f"{match.group(1)} WITH {match.group(2)}")
    rest = re.sub(r"[A-Za-z0-9.\-]+\s+WITH\s+[A-Za-z0-9.\-]+", " ", expression)
    for token in re.split(r"[^A-Za-z0-9.\-]+", rest):
        if token and token not in ("AND", "OR", "WITH"):
            found.add(token)
    return found


def id_for_filename(name: str, expression: str) -> str | None:
    """The SPDX id that a license file name stands for, or None when unclear."""
    remainder = normalized(name)
    for prefix in PREFIXES:
        if remainder.startswith(prefix):
            remainder = remainder[len(prefix):].lstrip("-")
            break
    tokens = set(remainder.split("-"))
    if "APACHE" in tokens and "LLVM" in tokens:
        return "Apache-2.0 WITH LLVM-exception"
    if "MIT" in tokens and "0" in tokens:
        return "MIT-0"
    if "MIT" in tokens:
        return "MIT"
    if "APACHE" in tokens:
        return "Apache-2.0"
    if "0BSD" in tokens or ("ZERO" in tokens and "BSD" in tokens):
        return "0BSD"
    if "BSD" in tokens and "3" in tokens:
        return "BSD-3-Clause"
    if "ZLIB" in tokens:
        return "Zlib"
    if "ISC" in tokens:
        return "ISC"
    if "MPL" in tokens or "MOZILLA" in tokens:
        return "MPL-2.0"
    if "UNICODE" in tokens:
        return "Unicode-3.0"
    if "CC0" in tokens:
        return "CC0-1.0"
    if "UNLICENSE" in tokens:
        return "Unlicense"
    if "GPL" in tokens:
        return "GPL-3.0-or-later"
    # A plain LICENSE or COPYING file stands for a lone license id.
    return sole_id(expression)


def clean_text(text: str) -> str:
    """One spelling of a license text: LF endings, no trailing blank lines."""
    return text.replace("\r\n", "\n").replace("\r", "\n").strip()


def spdx_text(identifier: str) -> str:
    """The committed standard text for an SPDX identifier."""
    path = SPDX_TEXT_DIR / f"{identifier}.txt"
    if not path.is_file():
        raise SystemExit(f"missing standard SPDX text: {path}")
    return clean_text(path.read_text(encoding="utf-8"))


def license_files(directory: Path) -> list[Path]:
    """The license, notice, and copyright files at a component's top level.

    The top level and the `license/` and `licenses/` subdirectories are read,
    because both layouts are common.
    """
    if not directory.is_dir():
        return []
    candidates = [path for path in sorted(directory.iterdir()) if path.is_file()]
    for name in ("license", "licenses"):
        subdirectory = directory / name
        if subdirectory.is_dir():
            candidates += [path for path in sorted(subdirectory.iterdir()) if path.is_file()]
    return [path for path in candidates if LICENSE_FILE.match(path.name)]


def nested_license_files(directory: Path) -> list[Path]:
    """The license, notice, and copyright documents below a component root.

    Some components keep a notice beside the data or code it covers, such as
    `src/unicode_tables/LICENSE-UNICODE`. The walk reads those. It skips the
    test, bench, example, and fixture folders, whose files cover inputs rather
    than the shipped code, and it does not follow a directory symlink, so a
    dependency's `node_modules` link never pulls in a second package.
    """
    if not directory.is_dir():
        return []
    found: list[Path] = []
    for root, directories, filenames in os.walk(directory, followlinks=False):
        directories[:] = [
            name
            for name in directories
            if name.lower() not in EXCLUDED_DIRS and not (Path(root) / name).is_symlink()
        ]
        for filename in sorted(filenames):
            if NESTED_LICENSE_FILE.match(filename):
                found.append(Path(root) / filename)
    return found


def read_text(path: Path) -> str:
    """The cleaned text of a license file."""
    return clean_text(path.read_text(encoding="utf-8", errors="replace"))


def fallback_ids(expression: str) -> list[str]:
    """The one identifier used for a component that ships no license file.

    The permissive ids come first. An id with no committed standard text, such
    as `LGPL-2.1-or-later`, is skipped: a dual-licensed crate is distributed
    under one of its other alternatives.
    """
    ids = named_ids(expression)
    for preferred in FALLBACK_PREFERENCE:
        if preferred in ids and (SPDX_TEXT_DIR / f"{preferred}.txt").is_file():
            return [preferred]
    return []


def add_text(groups: dict, text: str, *, component, filename=None, spdx=None, no_file=False) -> None:
    """Record that a component ships a text, or is covered by a standard one."""
    entry = groups.setdefault(text, {"shipped": set(), "no_file": set(), "spdx": set()})
    if no_file:
        entry["no_file"].add(component)
    else:
        entry["shipped"].add((component[0], component[1], filename))
    if spdx:
        entry["spdx"].add(spdx)


def component_texts(groups: dict, component: dict) -> None:
    """Add one component's own license, notice, and copyright texts.

    A component that ships any such document never gets a standard SPDX text:
    the generic fallback could replace the attribution the component supplies.
    """
    key = (component["name"], component["version"])
    top = license_files(component["directory"])
    seen = set(top)
    labelled = [(path.name, path) for path in top]
    excluded = EXCLUDED_NESTED.get(component["name"], frozenset())
    for path in nested_license_files(component["directory"]):
        if path in seen:
            continue
        seen.add(path)
        label = path.relative_to(component["directory"]).as_posix()
        if label in excluded:
            continue
        labelled.append((label, path))
    if labelled:
        for label, path in labelled:
            text = read_text(path)
            if text:
                add_text(
                    groups,
                    text,
                    component=key,
                    filename=label,
                    spdx=id_for_filename(path.name, component["license"]),
                )
        return
    for identifier in fallback_ids(component["license"]):
        add_text(groups, spdx_text(identifier), component=key, spdx=identifier, no_file=True)


def vendored_texts(groups: dict, components: list[dict]) -> None:
    """Add the license files of the native libraries a crate vendors and links."""
    by_name = {component["name"]: component for component in components}
    for crate, entries in VENDORED_NATIVE.items():
        component = by_name.get(crate)
        if component is None:
            continue
        for relative, spdx in entries:
            path = component["directory"] / relative
            if not path.is_file():
                raise SystemExit(f"vendored native license missing: {path}")
            add_text(
                groups,
                read_text(path),
                component=(component["name"], component["version"]),
                filename=relative,
                spdx=spdx,
            )
    sqlite = by_name.get("libsqlite3-sys")
    if sqlite is not None:
        add_text(
            groups,
            clean_text(SQLITE_BLESSING),
            component=(sqlite["name"], sqlite["version"]),
            filename="sqlite3/sqlite3.c (public domain)",
        )


def runtime_documents(tag_dir: Path) -> list[tuple[str, str]]:
    """The bundled runtime's own documents, read from the pinned archive.

    Returns `(label, text)` pairs for the runtime `LICENSE` and
    `ThirdPartyNotices.txt`. It fails when either document is missing, so a
    broken or changed archive never silently drops attribution.
    """
    required = ("LICENSE", "ThirdPartyNotices.txt")
    found: dict[str, Path] = {}
    for directory in sorted(path for path in tag_dir.glob("*/") if path.is_dir()):
        for name in required:
            candidate = directory / name
            if name not in found and candidate.is_file():
                found[name] = candidate
    missing = [name for name in required if name not in found]
    if missing:
        raise SystemExit(
            f"ONNX Runtime {RUNTIME_VERSION} document missing under {tag_dir}: "
            f"{', '.join(missing)}"
        )
    return [(name, read_text(found[name])) for name in required]


def unique_components(components: list[dict]) -> list[dict]:
    """The components, deduplicated and sorted by name and version."""
    seen = set()
    result = []
    for component in components:
        key = (component["name"], component["version"])
        if key in seen:
            continue
        seen.add(key)
        result.append(component)
    return sorted(result, key=lambda item: (item["name"].lower(), item["version"]))


def component_table(components: list[dict], extra: str | None = None) -> list[str]:
    """A markdown table for a component list."""
    header = "| Component | Version | License |"
    rule = "| --- | --- | --- |"
    if extra is not None:
        header = f"| Component | Version | License | {extra} |"
        rule = "| --- | --- | --- | --- |"
    lines = [header, rule]
    for component in components:
        row = f"| {component['name']} | {component['version']} | {component['license']} |"
        if extra is not None:
            row = (
                f"| {component['name']} | {component['version']} | "
                f"{component['license']} | {component['note']} |"
            )
        lines.append(row)
    return lines


def fence_for(text: str) -> str:
    """A code fence longer than any backtick run in the text."""
    fence = "```"
    while fence in text:
        fence += "`"
    return fence


def render_group(text: str, entry: dict) -> list[str]:
    """One license text with the components that it covers."""
    title = ", ".join(sorted(entry["spdx"])) or "License text"
    lines = [f"### {title}", ""]
    shipped = sorted(entry["shipped"], key=lambda item: (item[0].lower(), item[1], item[2]))
    if shipped:
        lines.append("Components covered by this text:")
        lines.append("")
        for name, version, filename in shipped:
            lines.append(f"- {name} {version} (`{filename}`)")
        lines.append("")
    no_file = sorted(entry["no_file"], key=lambda item: (item[0].lower(), item[1]))
    if no_file:
        lines.append("Components that ship no license file, covered by the standard text:")
        lines.append("")
        for name, version in no_file:
            lines.append(f"- {name} {version}")
        lines.append("")
    fence = fence_for(text)
    lines.append(fence)
    lines.append(text)
    lines.append(fence)
    lines.append("")
    return lines


def build_notices(*, rust=None, scripts=None, runtime=None) -> str:
    """The full contents of THIRD_PARTY_NOTICES.md.

    The three keyword arguments override the real inputs, so the tests can
    build the notice from fixtures.
    """
    if rust is None:
        rust = unique_components(rust_components(cargo_metadata()))
    if scripts is None:
        scripts = unique_components(pnpm_components())
    if runtime is None:
        runtime = runtime_documents(RUNTIME_DIR)
    bundled = sorted(BUNDLED, key=lambda item: item["name"].lower())

    groups: dict[str, dict] = {}
    for component in rust + scripts:
        component_texts(groups, component)
    vendored_texts(groups, rust)
    for label, text in runtime:
        add_text(
            groups,
            clean_text(text),
            component=("onnxruntime", RUNTIME_VERSION),
            filename=label,
            spdx="MIT" if label == "LICENSE" else None,
        )

    identifiers = set()
    for component in rust + scripts + bundled:
        identifiers |= named_ids(component["license"])
    text_ids = set()
    for entry in groups.values():
        text_ids |= entry["spdx"]
    other = sorted(identifiers - text_ids)

    lines = [
        "# Third-party notices",
        "",
        "Melomae is licensed under the GNU General Public License, version 3 only",
        "(see `LICENSE`). This file lists each third-party component that Melomae",
        "distributes or links, with the license of the component and the license and",
        "notice texts that the component ships.",
        "",
        "The gate keeps this file current. `pnpm verify` fails when the file does not",
        "match the dependency trees.",
        "",
        "This file is generated. Do not edit it by hand. Run",
        "`python tools/generate_notices.py` to write it again.",
        "",
        "Each text below is reproduced in full. Components that ship the same text",
        "share one copy. A component that ships no license file is listed under the",
        "standard text for one of its declared identifiers.",
        "",
        "The bundled ONNX Runtime release ships the same `LICENSE` and",
        "`ThirdPartyNotices.txt` on Windows and Linux. Both are reproduced once, in",
        "full, below.",
        "",
        "## Components",
        "",
        "### Rust crates",
        "",
        *component_table(rust),
        "",
        "### JavaScript packages",
        "",
        *component_table(scripts),
        "",
        "### Bundled components with no package manifest",
        "",
        *component_table(bundled, extra="Note"),
        "",
        "## License and notice texts",
        "",
    ]
    ordered = sorted(groups.items(), key=lambda item: (", ".join(sorted(item[1]["spdx"])).lower(), item[0]))
    for text, entry in ordered:
        lines.extend(render_group(text, entry))
    if other:
        lines.append("### Other licenses")
        lines.append("")
        lines.append("The license text for each identifier below is at the given URL.")
        lines.append("")
        for identifier in other:
            url = LICENSE_URLS.get(identifier, "(no canonical URL recorded)")
            lines.append(f"- {identifier}: {url}")
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail when the committed notice differs from the generated one",
    )
    arguments = parser.parse_args(argv)
    generated = build_notices()

    if not arguments.check:
        OUTPUT.write_text(generated, encoding="utf-8")
        print(f"wrote {OUTPUT.name}")
        return 0

    committed = clean_text(OUTPUT.read_text(encoding="utf-8")) if OUTPUT.is_file() else ""
    if committed != clean_text(generated):
        sys.stderr.write(
            f"{OUTPUT.name} is stale. Run `python tools/generate_notices.py` "
            "and commit the result.\n"
        )
        return 1
    print(f"{OUTPUT.name} is current")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
