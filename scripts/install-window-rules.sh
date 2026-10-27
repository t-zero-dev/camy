#!/usr/bin/env bash
# Copy plugin-owned Hyprland rules into ~/.config/hypr/camy.lua and require
# them from hyprland.lua. Idempotent. Does not rewrite the rest of the file.
set -euo pipefail
cd "$(dirname "$0")/.."

die() { printf '%s\n' "$@" >&2; exit 1; }

src=$PWD/hypr/camy.lua
[[ -f $src ]] || die "missing $src"

hypr_dir=${CAMY_HYPR_DIR:-$HOME/.config/hypr}
[[ $hypr_dir == /* ]] || die "hypr dir is not an absolute path"
case $hypr_dir in
  /tmp|/tmp/*|/var/tmp|/var/tmp/*|/dev/shm|/dev/shm/*) die "hypr dir $hypr_dir is shared tmp" ;;
esac
resolved=$(realpath -m -- "$hypr_dir")
case $resolved in
  /tmp|/tmp/*|/var/tmp|/var/tmp/*|/dev/shm|/dev/shm/*) die "hypr dir resolves to shared tmp $resolved" ;;
esac

mkdir -p -- "$hypr_dir"
dest=$hypr_dir/camy.lua
[[ ! -L $dest ]] || die "camy.lua dest is a symlink"

changed=0
if [[ ! -f $dest ]] || ! cmp -s -- "$src" "$dest"; then
  tmp=$hypr_dir/camy.lua.new
  rm -f -- "$tmp"
  install -m 644 -- "$src" "$tmp"
  mv -f -- "$tmp" "$dest"
  changed=1
fi

lua=$hypr_dir/hyprland.lua
marker='camy-window-rules'
block=$'-- camy-window-rules: begin\npcall(require, "hypr.camy")\n-- camy-window-rules: end\n'
if [[ -f $lua && ! -L $lua ]]; then
  if ! grep -q "$marker" "$lua" && ! grep -q 'hypr.camy' "$lua"; then
    printf '\n%s' "$block" >> "$lua"
    changed=1
  fi
fi

if [[ $changed == 1 && ${CAMY_SKIP_HYPR_RELOAD:-} != 1 && -x /usr/bin/hyprctl ]]; then
  /usr/bin/hyprctl reload >/dev/null
fi
printf 'Installed %s\n' "$dest"
