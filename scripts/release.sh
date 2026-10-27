#!/usr/bin/env bash
# Maintainer release stages. Writes locally; never commits, merges, or pushes
# unless the tag stage is given --push. See docs/RELEASING.md.
set -euo pipefail
cd "$(dirname "$0")/.."

die() { printf '%s\n' "$@" >&2; exit 1; }
usage() {
  cat >&2 <<'EOF'
Usage:
  bash scripts/release.sh helper prepare <X.Y.Z>
  bash scripts/release.sh helper tag [--push]
  bash scripts/release.sh helper pin helper-<X.Y.Z>
  bash scripts/release.sh plugin prepare <X.Y.Z>
  bash scripts/release.sh plugin tag [--push]
EOF
  exit 1
}

require_clean() {
  git rev-parse --abbrev-ref HEAD >/dev/null
  [[ $(git rev-parse --abbrev-ref HEAD) == main ]] || die "Run from main"
  [[ -z $(git status --porcelain) ]] || die "Working tree is not clean"
}

bump_helper() {
  local version=$1
  [[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "Version must be X.Y.Z"
  local current
  current=$(awk -F'"' '/^version = /{print $2; exit}' helper/Cargo.toml)
  [[ -n $current ]] || die "Could not read helper/Cargo.toml version"
  python3 - "$current" "$version" <<'PY' || die "helper version must increase"
import sys
from pathlib import Path
cur, new = sys.argv[1], sys.argv[2]
def parts(s): return tuple(int(x) for x in s.split("."))
if parts(new) <= parts(cur):
    sys.exit(1)
text = Path("helper/Cargo.toml").read_text()
old = f'version = "{cur}"'
repl = f'version = "{new}"'
if old not in text:
    sys.exit(1)
Path("helper/Cargo.toml").write_text(text.replace(old, repl, 1))
PY
  (cd helper && cargo update --workspace --offline >/dev/null 2>&1 || cargo generate-lockfile)
  printf 'Helper version %s -> %s in helper/Cargo.toml (not committed).\n' "$current" "$version"
}

bump_plugin() {
  local version=$1
  [[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "Version must be X.Y.Z"
  python3 - "$version" <<'PY' || die "Could not bump manifest.json"
import json, sys
from pathlib import Path
version = sys.argv[1]
path = Path("manifest.json")
data = json.loads(path.read_text())
cur = str(data.get("version") or "")
def parts(s): return tuple(int(x) for x in s.split("."))
if parts(version) <= parts(cur):
    sys.exit(1)
data["version"] = version
path.write_text(json.dumps(data, indent=2) + "\n")
print(f"Plugin version {cur} -> {version} in manifest.json (not committed).")
PY
}

tag_helper() {
  require_clean
  local version push=0
  version=$(awk -F'"' '/^version = /{print $2; exit}' helper/Cargo.toml)
  [[ -n $version ]] || die "Could not read helper version"
  local tag=helper-$version
  git rev-parse "$tag" >/dev/null 2>&1 && die "Tag $tag already exists"
  git tag -a "$tag" -m "Helper $version"
  printf 'Created annotated tag %s locally.\n' "$tag"
  if [[ ${1:-} == --push ]]; then
    git push github "$tag"
    printf 'Pushed %s to github. Helper release CI will draft assets.\n' "$tag"
  else
    printf 'Review the tag, then: git push github %s\n' "$tag"
  fi
}

tag_plugin() {
  require_clean
  local version
  version=$(python3 -c 'import json; print(json.load(open("manifest.json"))["version"])')
  local tag=v$version
  git rev-parse "$tag" >/dev/null 2>&1 && die "Tag $tag already exists"
  git tag -a "$tag" -m "CamY $version"
  printf 'Created annotated tag %s locally.\n' "$tag"
  if [[ ${1:-} == --push ]]; then
    git push github "$tag"
  else
    printf 'Review the tag, then: git push github %s\n' "$tag"
  fi
}

[[ $# -ge 2 ]] || usage
kind=$1
stage=$2
shift 2
case $kind-$stage in
  helper-prepare) [[ $# == 1 ]] || usage; bump_helper "$1" ;;
  helper-tag) tag_helper "${1:-}" ;;
  helper-pin) [[ $# == 1 ]] || usage; bash scripts/pin-helper.sh "$1" ;;
  plugin-prepare) [[ $# == 1 ]] || usage; bump_plugin "$1" ;;
  plugin-tag) tag_plugin "${1:-}" ;;
  *) usage ;;
esac
