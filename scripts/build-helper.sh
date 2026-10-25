#!/usr/bin/env bash
# Build the CamY helper into a user-owned cache and install it into bin/.
# Cargo's target directory must stay outside the plugin tree (omarchy plugin
# validate rejects symlinks) and must not be a shared tmp path: a precreated
# /tmp/camy-target can be replaced by another local account before cp.
set -euo pipefail

fail() {
  printf 'build-helper: %s\n' "$*" >&2
  exit 1
}

is_shared_tmp() {
  case "$1" in
    /tmp|/tmp/*|/var/tmp|/var/tmp/*|/dev/shm|/dev/shm/*) return 0 ;;
  esac
  return 1
}

check_private_dir() {
  local p="$1" uid mode
  [ -d "$p" ] || fail "$p is not a directory"
  [ ! -L "$p" ] || fail "$p is a symlink"
  uid=$(id -u)
  [ "$(stat -c %u "$p")" = "$uid" ] || fail "$p is not owned by uid $uid"
  mode=$(stat -c %a "$p")
  [ "$((8#$mode & 8#022))" -eq 0 ] || fail "$p is group/world-writable (mode $mode)"
}

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
PLUGIN=$(cd "$SCRIPT_DIR/.." && pwd -P)
is_shared_tmp "$PLUGIN" && fail "plugin path $PLUGIN is shared tmp"

CACHE_ROOT="${XDG_CACHE_HOME:-$HOME/.cache}"
[ -n "$CACHE_ROOT" ] || fail "HOME/XDG_CACHE_HOME is unset"
case "$CACHE_ROOT" in
  /*) ;;
  *) fail "cache root $CACHE_ROOT is not an absolute path" ;;
esac
is_shared_tmp "$CACHE_ROOT" && fail "cache root $CACHE_ROOT is shared tmp"
if [ -e "$CACHE_ROOT" ]; then
  CACHE_ROOT=$(cd "$CACHE_ROOT" && pwd -P)
  is_shared_tmp "$CACHE_ROOT" && fail "cache root $CACHE_ROOT is shared tmp"
fi

CACHE="$CACHE_ROOT/camy"
TARGET="$CACHE/target"
is_shared_tmp "$CACHE" && fail "cache $CACHE is shared tmp"
is_shared_tmp "$TARGET" && fail "target $TARGET is shared tmp"

BIN=$(awk '
  $0 == "[package]" { p=1; next }
  /^\[/ { p=0 }
  p && $1 == "name" {
    gsub(/"/, "", $3)
    print $3
    exit
  }
' "$PLUGIN/helper/Cargo.toml")
[ -n "$BIN" ] || fail "could not read package name from helper/Cargo.toml"
case "$BIN" in
  *[!a-zA-Z0-9_-]*|"") fail "refusing helper name '$BIN'" ;;
esac

umask 077
install -d -m 700 "$CACHE"
check_private_dir "$CACHE"
install -d -m 700 "$TARGET"
check_private_dir "$TARGET"

export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_DIR="$TARGET"
command -v cargo >/dev/null || fail "cargo not on PATH; install Rust from https://rustup.rs/"

cd "$PLUGIN/helper"
cargo build --release

SRC="$TARGET/release/$BIN"
[ -f "$SRC" ] || fail "missing $SRC"
[ ! -L "$SRC" ] || fail "$SRC is a symlink"
uid=$(id -u)
[ "$(stat -c %u "$SRC")" = "$uid" ] || fail "$SRC is not owned by uid $uid"
mode=$(stat -c %a "$SRC")
[ "$((8#$mode & 8#022))" -eq 0 ] || fail "$SRC is group/world-writable (mode $mode)"
check_private_dir "$(dirname "$SRC")"

DEST_DIR="$PLUGIN/bin"
install -d -m 700 "$DEST_DIR"
check_private_dir "$DEST_DIR"
DEST="$DEST_DIR/$BIN"
NEW="$DEST.new"
rm -f "$NEW"
install -m 700 "$SRC" "$NEW"
mv -f "$NEW" "$DEST"
printf 'build-helper: installed %s\n' "$DEST"
