#!/usr/bin/env bash
# After a public helper-* GitHub Release exists, download both assets, verify
# they are ELF executables, and write helper/release.pin. Does not commit.
set -euo pipefail
cd "$(dirname "$0")/.."

die() { printf '%s\n' "$@" >&2; exit 1; }
tag=${1:-}
[[ $tag == helper-* ]] || die "Usage: bash scripts/pin-helper.sh helper-<version>"

repo=t-zero-dev/camy
install -d -m 700 -- "${XDG_CACHE_HOME:-$HOME/.cache}/camy"
work=$(mktemp -d "${XDG_CACHE_HOME:-$HOME/.cache}/camy/pin.XXXXXX")
trap 'rm -rf "$work"' EXIT

pin=$work/release.pin
{
  printf '# Verified public helper release; update through scripts/release.sh helper pin.\n'
  printf 'tag=%s\n' "$tag"
  printf 'repo=%s\n' "$repo"
} > "$pin"

for arch in x86_64 aarch64; do
  asset=camy-helper-$arch-unknown-linux-gnu
  url=https://github.com/$repo/releases/download/$tag/$asset
  dest=$work/$asset
  curl -fsSL --proto https --proto-redir https \
    --connect-timeout 10 --max-time 30 --retry 2 --retry-max-time 30 \
    --max-filesize 4194304 \
    -A "camy/$tag (https://github.com/t-zero-dev/camy)" -o "$dest" -- "$url" \
    || die "Could not download $url. Publish GitHub Release $tag first."
  sz=$(stat -c %s -- "$dest")
  [[ $sz -gt 0 && $sz -lt 4194304 ]] || die "$asset exceeds 4 MiB cap or is empty"
  [[ $(head -c 4 "$dest") == $'\x7fELF' ]] || die "$asset is not an ELF executable"
  sum=$(sha256sum -- "$dest" | awk '{print $1}')
  printf 'asset_%s=%s\n' "$arch" "$asset" >> "$pin"
  printf 'sha256_%s=%s\n' "$arch" "$sum" >> "$pin"
done

# shellcheck source=helper-pin.sh
source scripts/helper-pin.sh
read_helper_pin "$pin"
install -m 644 "$pin" helper/release.pin
printf 'Wrote helper/release.pin for %s (not committed).\n' "$tag"
