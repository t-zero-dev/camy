#!/usr/bin/env bash
# Internal CI operation: build one optimized native asset and its metadata.
# Does not publish, tag, or push.
set -euo pipefail
cd "$(dirname "$0")/.."

die() { printf '%s\n' "$@" >&2; exit 1; }
[[ $# == 0 ]] || die "Usage: bash scripts/build-helper-release.sh"

if ! command -v rustc >/dev/null && [[ -x .tools/cargo/bin/rustc ]]; then
  export RUSTUP_HOME="$PWD/.tools/rustup" CARGO_HOME="$PWD/.tools/cargo"
  export PATH="$CARGO_HOME/bin:$PATH"
fi
command -v rustc >/dev/null || die "rustc not on PATH; install Rust from https://rustup.rs/"
host=$(rustc -vV | awk '/^host:/{print $2}')
case $host in
  x86_64-unknown-linux-gnu|aarch64-unknown-linux-gnu) ;;
  *) die "Build on x86_64-unknown-linux-gnu or aarch64-unknown-linux-gnu (host is $host)." ;;
esac

export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$PWD/target}
# Explicit target avoids a Cargo config/env target silently changing the asset.
cargo build --manifest-path helper/Cargo.toml --release --target "$host" --locked
src=$CARGO_TARGET_DIR/$host/release/camy
[[ -x $src ]] || die "cargo did not produce $src"
[[ ! -L $src ]] || die "$src is a symlink"

mkdir -p target/dist
asset=camy-helper-$host
dest=target/dist/$asset
cp -- "$src" "$dest"
if command -v strip >/dev/null; then
  strip --strip-unneeded -- "$dest" || true
fi
chmod 755 -- "$dest"
sum=$(sha256sum -- "$dest" | awk '{print $1}')
printf '%s  %s\n' "$sum" "$dest"

version=$(awk -F'"' '/^version = /{print $2; exit}' helper/Cargo.toml)
if command -v jq >/dev/null; then
  jq -n --arg source "$(git rev-parse HEAD)" --arg version "$version" \
    --arg asset "$asset" --arg sha256 "$sum" \
    '{source:$source, version:$version, asset:$asset, sha256:$sha256}' > "$dest.build.json"
else
  printf '{"source":"%s","version":"%s","asset":"%s","sha256":"%s"}\n' \
    "$(git rev-parse HEAD)" "$version" "$asset" "$sum" > "$dest.build.json"
fi
printf 'Native build: %s (tracked pin unchanged).\n' "$dest"
