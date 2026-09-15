# YoloCam for Omarchy

Control a [YoloLiv](https://www.yololiv.com/) YoloCam **S7** or **S3** from the Omarchy bar — exposure, focus, image, HDMI, and audio — the way YoloLiv Compose does on Windows.

The camera has no body menus. Settings are stored on the camera. This plugin is the remote.

## Features

- Bar glyph while a camera is on USB 3: accent for S7, a hue-shifted accent for S3, stacked glyphs when both are docked (selected model in front)
- Tabbed panel: Video, Image, Focus, Zoom, Exposure, White balance, Audio, Status
- Click-to-start MJPEG preview while the panel is open (1280×720). If OBS or another app already holds the UVC node, preview reports busy and the rest of the panel still works
- At most one S7 and one S3. A second body of the same model is ignored
- Settings persist on camera flash
- IPC: `omarchy-shell yolocam toggle`, `iso`, `shutter`, `focusMode`, `zoom`

Not included: firmware flashing, CLUT colour-wheel upload, YoloLiv remote assistance.

## Requirements

- [Omarchy](https://omarchy.org/) (Quickshell bar)
- YoloCam S7 and/or S3, powered, original USB 3 **data** cable
- Helper binary at `bin/yolocam` (built from `helper/`)

## Install

Place this directory at `~/.config/omarchy/plugins/yolocam`, build the helper, then enable the widget:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_DIR=/tmp/yolocam-target
cd ~/.config/omarchy/plugins/yolocam/helper
cargo build --release
cp -f /tmp/yolocam-target/release/yolocam ../bin/yolocam

omarchy plugin validate ~/.config/omarchy/plugins/yolocam
omarchy plugin enable yolocam --section right
```

Keep Cargo’s `target/` **outside** the plugin tree. `omarchy plugin validate` rejects symlinks, and `target/` is large.

## Use

Click the bar glyph to open the panel. With both cameras plugged in, pick S3 or S7 under the hero. Preview starts when you click the viewfinder; it stops when the panel closes.

IPC from a terminal:

```bash
omarchy-shell yolocam toggle
omarchy-shell yolocam iso 400
omarchy-shell yolocam shutter 16666    # microseconds; 16666 is 1/60 s
omarchy-shell yolocam focusMode 4      # 1 AFS, 2 manual, 3 AFC, 4 face
omarchy-shell yolocam zoom 50          # raw 50–98 (1.0×–4.0×)
```

Helper (watch already holds TCP 12345 while the bar is up):

```bash
BIN=~/.config/omarchy/plugins/yolocam/bin/yolocam
$BIN status                  # USB inventory (skips TCP while watch is up)
$BIN ctl set iso 400
$BIN recover                 # restart camera control without a power-cycle
```

Direct `get` / `set` / `probe` open a second TCP client — do not run them while watch owns port 12345. Full command list: [CLI](docs/cli.md).

## Hardware

| | S7 | S3 |
|---|---|---|
| USB VID:PID | `46f8:0851` | `46f8:0855` |
| USB gadget | CDC Ethernet | CDC NCM |
| Camera address | `192.168.129.10:12345` | `192.168.126.10:12345` |
| Video | UVC (MJPEG / H.264 / NV12 / YUY2, 1280×720 and up) | same sizes |

Leave the USB gadget off the default route. It stays that way when Wi-Fi or Ethernet already owns default.

If USB is present but control is refused on TCP 12345, the camera’s control process has stalled. Run `yolocam recover` or power-cycle data **and** power. See [troubleshooting](docs/troubleshooting.md).

## Documentation

| | |
|---|---|
| [CLI](docs/cli.md) | Helper commands |
| [Protocol](docs/protocol.md) | Yunxi control channel, named parameters, S3 vs S7 |
| [Architecture](docs/architecture.md) | Plugin, helper, preview, RPC |
| [Troubleshooting](docs/troubleshooting.md) | Recover, UVC busy, mixed dock |

## Related work

[yolocam-linux](https://gitlab.com/yolocam-linux/yolocam-linux) is a GPLv3 Python CLI and PySide6 GUI for the S3. This plugin does **not** vendor that code. It is an independent MIT implementation for Omarchy, using published hardware facts (USB IDs, gadget subnets, command IDs) and behaviour probed on S7 and S3 units.

## License

[MIT](LICENSE)
