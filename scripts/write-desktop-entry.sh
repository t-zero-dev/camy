#!/usr/bin/env bash
set -euo pipefail

die() { printf '%s\n' "$@" >&2; exit 1; }

is_shared_tmp() {
  case "$1" in
    /tmp|/tmp/*|/var/tmp|/var/tmp/*|/dev/shm|/dev/shm/*) return 0 ;;
  esac
  return 1
}

data_home=${XDG_DATA_HOME:-$HOME/.local/share}
[[ $data_home == /* ]] || die "XDG_DATA_HOME is not an absolute path"
is_shared_tmp "$data_home" && die "data home $data_home is shared tmp"
resolved=$(realpath -m -- "$data_home")
is_shared_tmp "$resolved" && die "data home resolves to shared tmp $resolved"

dir=$data_home/applications
mkdir -p -- "$dir"
[[ ! -L $dir ]] || die "applications dir is a symlink"
resolved_dir=$(realpath -e -- "$dir")
is_shared_tmp "$resolved_dir" && die "applications dir resolves to shared tmp $resolved_dir"
uid=$(id -u)
[[ $(stat -c %u -- "$dir") == "$uid" ]] || die "applications dir is not owned by uid $uid"

dest=$dir/camy.desktop
[[ ! -L $dest ]] || die "camy.desktop dest is a symlink"
tmp=$dir/camy.desktop.new
rm -f -- "$tmp"
cat > "$tmp" <<'DESK'
[Desktop Entry]
Type=Application
Name=CamY
Comment=Unofficial panel for YoloLiv YoloCam S7 and S3
Exec=omarchy-shell shell toggle io.github.t-zero-dev.camy '{}'
Icon=camera-photo
Terminal=false
Categories=AudioVideo;Photography;
StartupNotify=false
DBusActivatable=false
DESK
chmod 644 -- "$tmp"
mv -f -- "$tmp" "$dest"
echo "Wrote $dest"
