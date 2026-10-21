#!/usr/bin/env bash
set -euo pipefail
dir="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
mkdir -p "$dir"
cat > "$dir/yolocam.desktop" <<'DESK'
[Desktop Entry]
Type=Application
Name=YoloCam
Comment=Configure a YoloCam S7 or S3
Exec=omarchy-shell shell toggle yolocam '{}'
Icon=camera-photo
Terminal=false
Categories=AudioVideo;Photography;
StartupNotify=false
DBusActivatable=false
DESK
echo "Wrote $dir/yolocam.desktop"
