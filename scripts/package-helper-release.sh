#!/usr/bin/env bash
# Bundle native assets into SHA256SUMS and a candidate pin. Does not publish
# or write helper/release.pin (docs/RELEASING.md).
set -euo pipefail
cd "$(dirname "$0")/.."

die() { printf '%s\n' "$@" >&2; exit 1; }
dist=${1:-target/dist}
[[ -d $dist ]] || die "Usage: bash scripts/package-helper-release.sh target/dist"

version=$(awk -F'"' '/^version = /{print $2; exit}' helper/Cargo.toml)
[[ -n $version ]] || die "Could not read helper version"
tag=helper-$version

sums=$dist/SHA256SUMS
: > "$sums"
pin=$dist/release.pin
{
  printf '# Candidate pin from this package job; write the tracked pin with\n'
  printf '# bash scripts/release.sh helper pin %s after the release is public.\n' "$tag"
  printf 'tag=%s\n' "$tag"
  printf 'repo=t-zero-dev/camy\n'
} > "$pin"

for arch in x86_64 aarch64; do
  asset=camy-helper-$arch-unknown-linux-gnu
  file=$dist/$asset
  [[ -f $file ]] || die "Missing $file"
  [[ $(head -c 4 "$file") == $'\x7fELF' ]] || die "$file is not an ELF executable"
  sum=$(sha256sum -- "$file" | awk '{print $1}')
  printf '%s  %s\n' "$sum" "$asset" >> "$sums"
  printf 'asset_%s=%s\n' "$arch" "$asset" >> "$pin"
  printf 'sha256_%s=%s\n' "$arch" "$sum" >> "$pin"
done

printf 'Packaged %s (tracked pin unchanged).\n' "$dist"
