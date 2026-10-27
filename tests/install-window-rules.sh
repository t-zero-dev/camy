#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

die() { printf 'install-window-rules test: %s\n' "$*" >&2; exit 1; }

work=$(mktemp -d "${XDG_CACHE_HOME:-$HOME/.cache}/camy/hypr-test.XXXXXX")
trap 'rm -rf "$work"' EXIT
hypr=$work/hypr
mkdir -p "$hypr"
cat > "$hypr/hyprland.lua" <<'LUA'
require("default.hypr.omarchy")
LUA

export CAMY_HYPR_DIR=$hypr
export CAMY_SKIP_HYPR_RELOAD=1
bash scripts/install-window-rules.sh >/dev/null
[[ -f $hypr/camy.lua ]] || die "did not copy camy.lua"
grep -F -q 'title = "^CamY"' "$hypr/camy.lua" || die "copied rules missing title"
grep -F -q 'name = "camy-window"' "$hypr/camy.lua" || die "copied rules missing named window rule"
grep -F -q 'float = true' "$hypr/camy.lua" || die "copied rules missing float"
grep -F -q 'o.rebind("SUPER + T"' "$hypr/camy.lua" || die "copied rules missing Super+T wrap"
grep -q 'camy-window-rules: begin' "$hypr/hyprland.lua" || die "did not inject require"
grep -q 'pcall(require, "hypr.camy")' "$hypr/hyprland.lua" || die "missing pcall require"

# Second run is a no-op (one marker, same camy.lua).
before=$(cksum "$hypr/hyprland.lua")
bash scripts/install-window-rules.sh >/dev/null
n=$(grep -c 'camy-window-rules: begin' "$hypr/hyprland.lua" || true)
[[ $n == 1 ]] || die "injected the require twice"
[[ $(cksum "$hypr/hyprland.lua") == "$before" ]] || die "rewrote hyprland.lua on a no-op run"

# Shared tmp is refused.
if CAMY_HYPR_DIR=/tmp bash scripts/install-window-rules.sh >/dev/null 2>"$work/err"; then
  die "installed into /tmp"
fi
grep -q 'shared tmp' "$work/err" || die "tmp refusal did not name shared tmp"

# A symlink dest is refused.
linkdir=$work/linkhypr
mkdir -p "$linkdir"
ln -s "$hypr/camy.lua" "$linkdir/camy.lua"
if CAMY_HYPR_DIR=$linkdir bash scripts/install-window-rules.sh >/dev/null 2>"$work/err2"; then
  die "followed a symlink dest"
fi
grep -q 'symlink' "$work/err2" || die "symlink refusal did not name symlink"

printf 'install-window-rules tests passed\n'
