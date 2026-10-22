#!/usr/bin/env bash
set -euo pipefail
dir="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
mkdir -p "$dir"
cat > "$dir/omacam.desktop" <<'DESK'
[Desktop Entry]
Type=Application
Name=Omacam
Comment=Unofficial panel for YoloLiv YoloCam S7 and S3
Exec=omarchy-shell shell toggle io.github.t-zero-dev.omacam '{}'
Icon=camera-photo
Terminal=false
Categories=AudioVideo;Photography;
StartupNotify=false
DBusActivatable=false
DESK
echo "Wrote $dir/omacam.desktop"
