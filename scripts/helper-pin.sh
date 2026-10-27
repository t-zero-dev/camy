#!/usr/bin/env bash
# Shared pin reader for the installer and release builder. Data, never sourced.
helper_machine() {
  case $1 in
    x86_64) printf 'x86_64\n' ;;
    aarch64|arm64) printf 'aarch64\n' ;;
    *) die "Unsupported helper architecture: $1 (supported: x86_64, aarch64)." ;;
  esac
}

read_helper_pin() {
  local file=$1 line key val arch
  [[ -f $file ]] || die "CamY helper pin missing: $file"
  tag='' repo=''
  declare -gA assets=() hashes=()
  local -A seen=()
  while IFS= read -r line || [[ -n $line ]]; do
    [[ $line =~ ^[[:space:]]*(#|$) ]] && continue
    key=${line%%=*}
    val=${line#*=}
    [[ -n $key && $line == *=* ]] || die "Invalid pin line in $file: $line"
    [[ -z ${seen[$key]:-} ]] || die "Duplicate key in $file: $key"
    seen[$key]=1
    case $key in
      tag|repo) printf -v "$key" '%s' "$val" ;;
      asset_x86_64|asset_aarch64) assets[${key#asset_}]=$val ;;
      sha256_x86_64|sha256_aarch64) hashes[${key#sha256_}]=$val ;;
      *) die "Unknown key in $file: $key" ;;
    esac
  done < "$file"
  [[ -n $tag && -n $repo && ${#assets[@]} -gt 0 ]] || die "Incomplete pin in $file"
  [[ $tag =~ ^helper-[A-Za-z0-9._-]+$ ]] || die "Pin tag in $file must be helper-<version> without path"
  [[ $repo == t-zero-dev/camy ]] || die "Pin repo in $file must be t-zero-dev/camy"
  for arch in x86_64 aarch64; do
    [[ -n ${assets[$arch]:-}${hashes[$arch]:-} ]] || continue
    [[ ${assets[$arch]:-} == "camy-helper-$arch-unknown-linux-gnu" ]] \
      || die "Invalid $arch asset in $file"
    [[ ${hashes[$arch]:-} =~ ^[a-f0-9]{64}$ ]] \
      || die "Pin sha256 for $arch in $file is not 64 lowercase hex digits"
  done
}
