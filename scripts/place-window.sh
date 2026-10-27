#!/usr/bin/env bash
# Float, size, and centre the CamY window. Height defaults to 1220.
set -euo pipefail
h=${1:-1220}
case $h in
  ''|*[!0-9]*) h=1220 ;;
esac
if (( h < 720 )); then h=720; fi
if (( h > 1220 )); then h=1220; fi

if [[ -z ${HYPRLAND_INSTANCE_SIGNATURE:-} && -n ${XDG_RUNTIME_DIR:-} ]]; then
  for d in "$XDG_RUNTIME_DIR"/hypr/*; do
    [[ -S $d/.socket.sock ]] || continue
    sig=${d##*/}
    if HYPRLAND_INSTANCE_SIGNATURE=$sig /usr/bin/hyprctl -j monitors >/dev/null 2>&1; then
      export HYPRLAND_INSTANCE_SIGNATURE=$sig
      break
    fi
  done
fi

sel='title:^CamY$'
/usr/bin/hyprctl dispatch "hl.dsp.window.float({ action = \"on\", window = \"$sel\" })" >/dev/null
/usr/bin/hyprctl dispatch "hl.dsp.window.resize({ x = 720, y = $h, relative = false, window = \"$sel\" })" >/dev/null
/usr/bin/hyprctl dispatch "hl.dsp.window.center({ window = \"$sel\" })" >/dev/null
