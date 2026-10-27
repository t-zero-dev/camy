#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

die() { printf 'write-desktop-entry test: %s\n' "$*" >&2; exit 1; }

install -d -m 700 -- "${XDG_CACHE_HOME:-$HOME/.cache}/camy"
work=$(mktemp -d "${XDG_CACHE_HOME:-$HOME/.cache}/camy/desktop-test.XXXXXX")
trap 'rm -rf "$work"' EXIT
data=$work/data
mkdir -p "$data"
export XDG_DATA_HOME=$data

bash scripts/write-desktop-entry.sh >/dev/null
[[ -f $data/applications/camy.desktop ]] || die "did not write camy.desktop"
grep -q 'Name=CamY' "$data/applications/camy.desktop" || die "desktop missing Name"

victim=$work/victim
echo secret > "$victim"
rm -f -- "$data/applications/camy.desktop"
ln -s -- "$victim" "$data/applications/camy.desktop"
if bash scripts/write-desktop-entry.sh >/dev/null 2>"$work/err"; then
  die "followed a camy.desktop symlink"
fi
grep -q 'symlink' "$work/err" || die "symlink refusal did not name symlink"
[[ $(cat "$victim") == secret ]] || die "clobbered symlink target"

export XDG_DATA_HOME=/tmp
if bash scripts/write-desktop-entry.sh >/dev/null 2>"$work/err2"; then
  die "installed desktop into /tmp"
fi
grep -q 'shared tmp' "$work/err2" || die "tmp refusal did not name shared tmp"

printf 'write-desktop-entry tests passed\n'
