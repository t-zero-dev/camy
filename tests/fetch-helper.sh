#!/usr/bin/env bash
# Offline installer tests: pin format, hash match, hash mismatch, dest outside /tmp.
set -euo pipefail
cd "$(dirname "$0")/.."

die() { printf 'fetch-helper test: %s\n' "$*" >&2; exit 1; }

# Tracked pin is well-formed (tag, repo, at least one 64-hex arch).
bash -c 'source scripts/helper-pin.sh; die(){ printf "%s\n" "$@" >&2; exit 1; }; read_helper_pin helper/release.pin' \
  || die "tracked helper/release.pin did not parse"

install -d -m 700 -- "${XDG_CACHE_HOME:-$HOME/.cache}/camy"
work=$(mktemp -d "${XDG_CACHE_HOME:-$HOME/.cache}/camy/test.XXXXXX")
trap 'rm -rf "$work"' EXIT

# Pin reader rejects a bad hash.
bad=$work/bad.pin
cat > "$bad" <<'EOF'
tag=helper-0.0.0
repo=t-zero-dev/camy
asset_x86_64=camy-helper-x86_64-unknown-linux-gnu
sha256_x86_64=not-a-hash
EOF
if bash -c 'source scripts/helper-pin.sh; die(){ printf "%s\n" "$@" >&2; exit 1; }; read_helper_pin "'"$bad"'"' 2>/dev/null; then
  die "accepted a pin whose sha256 is not 64 hex digits"
fi

# A 64-byte ELF-looking blob with a real sha256.
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
mkdir -p "$data"
export XDG_DATA_HOME=$data
export XDG_CACHE_HOME=$work/cache
mkdir -p "$XDG_CACHE_HOME"
export CAMY_HELPER_PIN=$pin
export CAMY_HELPER_MACHINE=x86_64
export CAMY_HELPER_ASSET=$asset

got=$(bash scripts/fetch-helper.sh --print-path)
want=$data/camy/bin/camy
[[ $got == "$want" ]] || die "print-path was $got, expected $want"
[[ -x $want ]] || die "installed helper is not executable"
[[ $(sha256sum -- "$want" | awk '{print $1}') == "$sum" ]] || die "installed hash mismatch"

# Second fetch is a no-op (same hash).
got2=$(bash scripts/fetch-helper.sh --print-path)
[[ $got2 == "$want" ]] || die "second print-path changed path"

# Mismatch is refused and dest is left as the good file.
badasset=$work/bad.bin
printf '\x7fELFBAD' > "$badasset"
export CAMY_HELPER_ASSET=$badasset
rm -f -- "$want"
if bash scripts/fetch-helper.sh --print-path >/dev/null 2>"$work/err"; then
  die "accepted a sha256 mismatch"
fi
grep -q 'sha256 mismatch' "$work/err" || die "mismatch did not name sha256"
[[ ! -e $want ]] || die "mismatch left a dest file"

# Shared tmp as data home is refused.
export XDG_DATA_HOME=/tmp
export CAMY_HELPER_ASSET=$asset
if bash scripts/fetch-helper.sh --print-path >/dev/null 2>"$work/err2"; then
  die "installed into /tmp"
fi
grep -q 'shared tmp' "$work/err2" || die "tmp refusal did not name shared tmp"

printf 'fetch-helper tests passed\n'
