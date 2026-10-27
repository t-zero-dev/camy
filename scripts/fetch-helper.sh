#!/usr/bin/env bash
# Fetch the pinned GitHub Release asset, verify its committed sha256, and
# install it under $XDG_DATA_HOME/camy/bin (docs/RELEASING.md).
# Ordinary launch never calls this. Plugin bootstrap does, through
# `run.sh --ensure`, only when the checkout has no local helper.
set -euo pipefail
umask 077
cd "$(dirname "$0")/.."

die() { printf '%s\n' "$@" >&2; exit 1; }
# shellcheck source=helper-pin.sh
source scripts/helper-pin.sh

is_shared_tmp() {
  case "$1" in
    /tmp|/tmp/*|/var/tmp|/var/tmp/*|/dev/shm|/dev/shm/*) return 0 ;;
  esac
  return 1
}

check_private_dir() {
  local p=$1 uid mode
  [[ -d $p ]] || die "$p is not a directory"
  [[ ! -L $p ]] || die "$p is a symlink"
  uid=$(id -u)
  [[ $(stat -c %u -- "$p") == "$uid" ]] || die "$p is not owned by uid $uid"
  mode=$(stat -c %a -- "$p")
  [[ $((8#$mode & 8#022)) -eq 0 ]] || die "$p is group/world-writable (mode $mode)"
}

refuse_shared() {
  local p=$1 r
  is_shared_tmp "$p" && die "$p is shared tmp"
  r=$(realpath -m -- "$p")
  is_shared_tmp "$r" && die "$p resolves to shared tmp $r"
  printf '%s\n' "$r"
}

machine=$(helper_machine "${CAMY_HELPER_MACHINE:-$(uname -m)}")
pin_file=${CAMY_HELPER_PIN:-helper/release.pin}
read_helper_pin "$pin_file"
asset=${assets[$machine]:-}
sha256=${hashes[$machine]:-}
[[ -n $asset && -n $sha256 ]] || die "No pinned $machine helper for $tag yet. Publish its release asset and add its checksum to $pin_file."

data_home=${XDG_DATA_HOME:-$HOME/.local/share}
[[ $data_home == /* ]] || die "XDG_DATA_HOME is not an absolute path"
refuse_shared "$data_home" >/dev/null
dest_dir=$data_home/camy/bin
dest=$dest_dir/camy

hash_of() { sha256sum -- "$1" | awk '{print $1}'; }

# 4 MiB: today's helpers are ~0.9 MiB. Cap before the sha256 check so a
# swapped Release asset cannot fill the cache. head -c still applies when
# the server omits Content-Length.
MAX_HELPER_BYTES=4194304

refuse_oversize() {
  local f=$1 sz
  [[ -f $f ]] || die "helper asset is missing"
  sz=$(stat -c %s -- "$f")
  [[ $sz -gt 0 ]] || die "helper asset is empty"
  [[ $sz -lt $MAX_HELPER_BYTES ]] || die "helper asset exceeds $MAX_HELPER_BYTES byte cap"
}

if [[ -e $dest || -L $dest ]]; then
  [[ ! -L $dest ]] || die "helper dest is a symlink"
  if [[ -d $dest_dir ]]; then
    refuse_shared "$dest_dir" >/dev/null
    check_private_dir "$dest_dir"
  fi
  if [[ -x $dest ]] && [[ $(hash_of "$dest") == "$sha256" ]]; then
    [[ ${1:-} == --print-path ]] && printf '%s\n' "$dest"
    exit 0
  fi
fi

cache_root=${XDG_CACHE_HOME:-$HOME/.cache}
[[ $cache_root == /* ]] || die "cache root is not an absolute path"
refuse_shared "$cache_root" >/dev/null
install -d -m 700 -- "$cache_root/camy"
refuse_shared "$cache_root/camy" >/dev/null
check_private_dir "$cache_root/camy"
work=$(mktemp -d "$cache_root/camy/fetch.XXXXXX")
trap 'rm -rf "$work"' EXIT
tmp=$work/$asset

if [[ -n ${CAMY_HELPER_ASSET:-} ]]; then
  [[ -f $CAMY_HELPER_ASSET ]] || die "CAMY_HELPER_ASSET is not a file: $CAMY_HELPER_ASSET"
  refuse_oversize "$CAMY_HELPER_ASSET"
  cp -- "$CAMY_HELPER_ASSET" "$tmp"
else
  url=${CAMY_HELPER_URL:-https://github.com/$repo/releases/download/$tag/$asset}
  [[ $url == https://github.com/t-zero-dev/camy/* ]] \
    || die "CAMY_HELPER_URL must be an https://github.com/t-zero-dev/camy/ URL"
  set +e
  curl -fsSL --proto https --proto-redir https \
    --connect-timeout 10 --max-time 30 --retry 2 --retry-max-time 30 \
    --max-filesize "$MAX_HELPER_BYTES" \
    -A "camy/$tag (https://github.com/t-zero-dev/camy)" \
    -- "$url" 2>"$work/curl.err" | head -c "$MAX_HELPER_BYTES" > "$tmp"
  curl_rc=${PIPESTATUS[0]}
  set -e
  if [[ ! -s $tmp ]]; then
    die "Could not download $asset from $url." \
      "$(tr '\n' ' ' <"$work/curl.err" 2>/dev/null)" \
      "Publish GitHub Release $tag on $repo with that asset matching $pin_file, and make the repository public so the asset is anonymous." \
      "From a checkout with Rust: bash scripts/build-helper.sh"
  fi
  refuse_oversize "$tmp"
  if [[ $curl_rc -ne 0 && $curl_rc -ne 23 && $curl_rc -ne 141 ]]; then
    die "Could not download $asset from $url." \
      "Publish GitHub Release $tag on $repo with that asset matching $pin_file, and make the repository public so the asset is anonymous." \
      "From a checkout with Rust: bash scripts/build-helper.sh"
  fi
fi

got=$(hash_of "$tmp")
if [[ $got != "$sha256" ]]; then
  die "Helper sha256 mismatch for $asset." \
    "expected $sha256" \
    "got      $got" \
    "The committed pin is the source of truth; a substituted asset is refused."
fi

[[ $(head -c 4 "$tmp") == $'\x7fELF' ]] || die "Pinned helper is not an ELF executable."

install -d -m 700 -- "$dest_dir"
refuse_shared "$dest_dir" >/dev/null
check_private_dir "$dest_dir"
[[ ! -L $dest ]] || die "helper dest is a symlink"
chmod 755 -- "$tmp"
# Temp-and-rename so a half-written dest is never executable as the helper.
new=$dest_dir/camy.new
rm -f -- "$new"
mv -f -- "$tmp" "$new"
chmod 755 -- "$new"
mv -f -- "$new" "$dest"
trap 'rm -rf "$work"' EXIT

[[ ${1:-} == --print-path ]] && printf '%s\n' "$dest"
exit 0
