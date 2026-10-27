#!/usr/bin/env bash
# run.sh --print-path ignores unhashed helpers unless CAMY_USE_LOCAL_HELPER=1.
set -euo pipefail
cd "$(dirname "$0")/.."

die() { printf 'run-ensure test: %s\n' "$*" >&2; exit 1; }

install -d -m 700 -- "${XDG_CACHE_HOME:-$HOME/.cache}/camy"
work=$(mktemp -d "${XDG_CACHE_HOME:-$HOME/.cache}/camy/run-test.XXXXXX")
trap 'rm -rf "$work"' EXIT

asset=$work/camy-helper-x86_64-unknown-linux-gnu
printf '\x7fELF' > "$asset"
dd if=/dev/zero bs=60 count=1 >> "$asset" 2>/dev/null
sum=$(sha256sum -- "$asset" | awk '{print $1}')
pin=$work/release.pin
cat > "$pin" <<EOF
tag=helper-0.0.0
repo=t-zero-dev/camy
asset_x86_64=camy-helper-x86_64-unknown-linux-gnu
sha256_x86_64=$sum
asset_aarch64=camy-helper-aarch64-unknown-linux-gnu
sha256_aarch64=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
EOF

data=$work/data
mkdir -p "$data" "$work/hypr" "$work/cache"
printf 'require("default.hypr.omarchy")\n' > "$work/hypr/hyprland.lua"
export XDG_DATA_HOME=$data
export XDG_CACHE_HOME=$work/cache
export CAMY_HELPER_PIN=$pin
export CAMY_HELPER_MACHINE=x86_64
export CAMY_HELPER_ASSET=$asset
export CAMY_HYPR_DIR=$work/hypr
export CAMY_SKIP_HYPR_RELOAD=1

export CAMY_HELPER_BINARY=/usr/bin/true
got=$(bash run.sh --print-path)
want=$data/camy/bin/camy
[[ $got == "$want" ]] || die "unpinned CAMY_HELPER_BINARY won: $got"
[[ -x $want ]] || die "did not install hashed dest"

export CAMY_USE_LOCAL_HELPER=1
got=$(bash run.sh --print-path)
[[ $got == /usr/bin/true ]] || die "opt-in did not use CAMY_HELPER_BINARY: $got"

printf 'run-ensure tests passed\n'
