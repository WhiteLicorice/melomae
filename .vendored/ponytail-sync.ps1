# Copy one vendored skill from its pinned submodule into .agents/skills and
# .claude/skills.
#
#   pwsh -File .vendored/<name>-sync.ps1            copy the pinned commit
#   pwsh -File .vendored/<name>-sync.ps1 -Update    move the pin to the upstream tip, then copy
#
# The file name gives the submodule. <name>-sync.ps1 reads .vendored/<name>.

param([switch] $Update)

$ErrorActionPreference = 'Stop'

# The skill folders that this submodule ships.
$skills = @('ponytail', 'ponytail-audit', 'ponytail-debt', 'ponytail-gain', 'ponytail-help', 'ponytail-review')
# The folder in the submodule that holds those skill folders.
$agentsSource = 'skills'
# The same folder, or the Claude build when upstream ships one.
$claudeSource = 'skills'

$root = Split-Path -Parent $PSScriptRoot
$name = (Split-Path -Leaf $PSCommandPath) -replace '-sync\.ps1$', ''
$submodule = Join-Path $PSScriptRoot $name

if ($Update) {
  git -C $root submodule update --init --remote --depth 1 -- ".vendored/$name"
  if ($LASTEXITCODE -ne 0) {
    throw "${name}: git submodule update failed."
  }
}

# Check every source before the first delete.
foreach ($source in @($agentsSource, $claudeSource)) {
  foreach ($skill in $skills) {
    if (-not (Test-Path (Join-Path $submodule "$source/$skill/SKILL.md"))) {
      throw "${name}: .vendored/$name/$source/$skill/SKILL.md is missing. Run: git submodule update --init --depth 1 .vendored/$name"
    }
  }
}

function Copy-Skills {
  param([string] $Source, [string] $Harness)

  $target = Join-Path $root "$Harness/skills"
  New-Item -ItemType Directory -Force $target | Out-Null
  foreach ($skill in $skills) {
    $destination = Join-Path $target $skill
    if (Test-Path $destination) {
      Remove-Item -Recurse -Force $destination
    }
    Copy-Item -Recurse (Join-Path $submodule "$Source/$skill") $destination
  }
}

Copy-Skills -Source $agentsSource -Harness '.agents'
Copy-Skills -Source $claudeSource -Harness '.claude'

$commit = git -C $submodule rev-parse --short HEAD
Write-Output "${name}: synced $($skills.Count) skill folder(s) from $commit."
