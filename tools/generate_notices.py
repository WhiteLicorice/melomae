#!/usr/bin/env python3
"""Generate `THIRD_PARTY_NOTICES.md` from the dependency trees.

Melomae must ship a notice for every bundled component (SRS section 11). This
script builds that notice from three sources:

    - the Rust crates that the shipped build links (Cargo), normal and build
      dependencies for every target, without dev-dependencies;
    - the production pnpm dependencies (JavaScript);
    - the components that no package manifest lists: the ONNX Runtime library
      and the Apollo model (Stack section J).

For each component it reads the license text from the dependency source, when a
license file is present, and groups the texts by SPDX identifier.

    python tools/generate_notices.py
    python tools/generate_notices.py --check

`--check` writes nothing. It fails when the committed file differs, so a change
to the dependency trees must regenerate the notice. The gate runs that check.

The output is deterministic: it holds no path, no date, and no host name. The
two lockfiles fix the component set and the license texts. Standard library only.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
OUTPUT = REPO_ROOT / "THIRD_PARTY_NOTICES.md"

# The components that no package manifest lists but the installer bundles
# (Stack section J). The version of the model is the checkpoint revision.
BUNDLED = [
    {
        "name": "onnxruntime",
        "version": "1.28.3",
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

# The license text for an identifier that has no file in the dependency trees.
LICENSE_URLS = {
    "CC-BY-SA-4.0": "https://creativecommons.org/licenses/by-sa/4.0/legalcode.en",
    "CC0-1.0": "https://creativecommons.org/publicdomain/zero/1.0/legalcode.en",
    "MIT-0": "https://spdx.org/licenses/MIT-0.html",
    "LGPL-2.1-or-later": "https://www.gnu.org/licenses/old-licenses/lgpl-2.1.html",
}

LICENSE_FILE = re.compile(r"(?i)^(licen[cs]e|copying|unlicense|notice)")
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


def license_texts(component: dict) -> list[tuple[str, str]]:
    """The (spdx id, text) pairs for one component, read from its source."""
    directory = component["directory"]
    if not directory.is_dir():
        return []
    found = []
    for path in sorted(directory.iterdir()):
        if not path.is_file() or not LICENSE_FILE.match(path.name):
            continue
        if path.name.upper().startswith("NOTICE"):
            continue
        identifier = id_for_filename(path.name, component["license"])
        if identifier is None:
            continue
        try:
            text = clean_text(path.read_text(encoding="utf-8", errors="replace"))
        except OSError:
            continue
        if text:
            found.append((identifier, text))
    return found


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


def build_notices() -> str:
    """The full contents of THIRD_PARTY_NOTICES.md."""
    metadata = cargo_metadata()
    rust = unique_components(rust_components(metadata))
    scripts = unique_components(pnpm_components())
    bundled = sorted(BUNDLED, key=lambda item: item["name"].lower())

    # One text per identifier. Every crate repeats the license body with its own
    # copyright line, so a text per crate would repeat the same license hundreds
    # of times. The component table above names each crate; the text below is the
    # license itself. A crate that declares the identifier alone gives the clearest
    # text, so it wins.
    candidates: dict[str, list[tuple[bool, str, str]]] = {}
    identifiers = set()
    for component in rust + scripts:
        identifiers |= named_ids(component["license"])
        for identifier, text in license_texts(component):
            exact = sole_id(component["license"]) == identifier
            candidates.setdefault(identifier, []).append(
                (not exact, component["name"].lower(), component["version"], text)
            )
    for component in bundled:
        identifiers |= named_ids(component["license"])
    texts = {}
    for identifier, found in candidates.items():
        found.sort(key=lambda item: (item[0], item[1], item[2]))
        texts[identifier] = found[0][3]

    lines = [
        "# Third-party notices",
        "",
        "Melomae is licensed under the GNU General Public License, version 3 or later",
        "(see `LICENSE`). This file lists each third-party component that Melomae",
        "distributes or links, with the license of the component.",
        "",
        "The gate keeps this file current. `pnpm verify` fails when the file does not",
        "match the dependency trees.",
        "",
        "This file is generated. Do not edit it by hand. Run",
        "`python tools/generate_notices.py` to write it again.",
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
        "## License texts",
        "",
    ]
    for identifier in sorted(texts):
        lines.append(f"### {identifier}")
        lines.append("")
        lines.append("```")
        lines.append(texts[identifier])
        lines.append("```")
        lines.append("")
    missing = sorted(identifiers - set(texts))
    if missing:
        lines.append("### Other licenses")
        lines.append("")
        lines.append("The license text for each identifier below is at the given URL.")
        lines.append("")
        for identifier in missing:
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

    committed = OUTPUT.read_text(encoding="utf-8") if OUTPUT.is_file() else ""
    if committed != generated:
        sys.stderr.write(
            f"{OUTPUT.name} is stale. Run `python tools/generate_notices.py` "
            "and commit the result.\n"
        )
        return 1
    print(f"{OUTPUT.name} is current")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
