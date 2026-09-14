# YoloCam for Omarchy

A native Omarchy bar plugin that talks to YoloCam S7 (and S3, when present) over the same Yunxi protobuf control channel YoloLiv Compose uses on Windows.

The camera has no body menus. Settings live on the camera. This plugin is the remote.

The bar glyph colour is the model: accent = S7, a shifted accent = S3, two overlapping cameras = both. At most one of each model.

## What you get

- Bar glyph, live while a YoloCam is on USB 3
- Tabbed panel: Video, Image, Focus, Zoom, Exposure, WB, Audio, Status
- Live MJPEG preview **only while the panel is open**, 1280×720, ~15 fps, JPEG passthrough in a Rust helper
- If OBS already has the UVC node, preview says "in use" and controls keep working
- IPC: `omarchy-shell yolocam toggle` / `iso` / `shutter` / `focusMode` / `zoom`

Not included: firmware flashing, YoloLiv remote assistance, QtMultimedia inside `omarchy-shell`.

## Requirements

- Omarchy (Quickshell bar)
- The S7 powered, lens mounted, original USB 3 **data** cable
- Helper binary at `bin/yolocam` (built from `helper/`)

## Build the helper

Keep Cargo's `target/` **outside** the plugin tree (`omarchy plugin validate` rejects any symlink, and `target/` is large):

```bash
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_DIR=/tmp/yolocam-target
cd ~/.config/omarchy/plugins/yolocam/helper
cargo build --release
cp -f /tmp/yolocam-target/release/yolocam ../bin/yolocam
```

## CLI

```bash
~/.config/omarchy/plugins/yolocam/bin/yolocam status
yolocam get iso shutter-us focus-mode
yolocam set iso 400
yolocam shutter 1/60
yolocam watch
```

## Enable

```bash
omarchy plugin validate ~/.config/omarchy/plugins/yolocam
omarchy plugin enable yolocam --section right
```

## Hardware notes

This S7 is USB `46f8:0851`, gadget `192.168.129.10:12345`. An S3 is expected as `46f8:0855` / `192.168.126.10:12345` (unprobed until one is docked). The CDC link must not become the default route (it does not, as long as Wi-Fi/Ethernet already owns default).

If `yolocam status` shows USB but `"control": false` and connection refused on port 12345, the camera's control process has stalled. Power-cycle the S7 (data + power cables). Do not leave a second app STREAMON on `/dev/video34` while hammering GET-all.

Preview is click-to-start in the panel so it never fights OBS unless you ask.
