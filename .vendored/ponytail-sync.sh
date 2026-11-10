#!/usr/bin/env bash
# Copy one vendored skill from its pinned submodule into .agents/skills and
# .claude/skills.
#
#   bash .vendored/<name>-sync.sh            copy the pinned commit
#   bash .vendored/<name>-sync.sh --update   move the pin to the upstream tip, then copy
#
# The file name gives the submodule. <name>-sync.sh reads .vendored/<name>.

set -euo pipefail

# The skill folders that this submodule ships.
skills=(ponytail ponytail-audit ponytail-debt ponytail-gain ponytail-help ponytail-review)
# The folder in the submodule that holds those skill folders.
agents_source="skills"
# The same folder, or the Claude build when upstream ships one.
claude_source="skills"

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/.." && pwd)"
name="$(basename "${BASH_SOURCE[0]}" -sync.sh)"
submodule="$here/$name"

if [ "${1:-}" = "--update" ] && [ $# -eq 1 ]; then
  git -C "$root" submodule update --init --remote --depth 1 -- ".vendored/$name"
elif [ $# -gt 0 ]; then
  echo "Usage: bash .vendored/$name-sync.sh [--update]" >&2
  exit 2
fi

# Check every source before the first delete.
for source in "$agents_source" "$claude_source"; do
  for skill in "${skills[@]}"; do
    if [ ! -f "$submodule/$source/$skill/SKILL.md" ]; then
      echo "$name: .vendored/$name/$source/$skill/SKILL.md is missing." >&2
      echo "Run: git submodule update --init --depth 1 .vendored/$name" >&2
      exit 1
    fi
  done
done

copy_skills() {
  local source="$submodule/$1" target="$root/$2/skills" skill
  mkdir -p "$target"
  for skill in "${skills[@]}"; do
    rm -rf "${target:?}/$skill"
    cp -R "$source/$skill" "$target/$skill"
  done
}

copy_skills "$agents_source" .agents
copy_skills "$claude_source" .claude

echo "$name: synced ${#skills[@]} skill folder(s) from $(git -C "$submodule" rev-parse --short HEAD)."
