# YoloCam

Configure a [YoloLiv](https://www.yololiv.com/) YoloCam **S7** or **S3** from Omarchy — the way Compose does on Windows.

This page is the user guide: install, first run, and everyday use. How the helper is built lives in [CONTRIBUTING.md](CONTRIBUTING.md).

The camera has no body menus. Settings stay on the camera. This plugin is the remote.

Not included: firmware flashing, CLUT colour-wheel upload, YoloLiv remote assistance.

## Install

Omarchy 4. Needs [Rust](https://rustup.rs/) to build the helper once.

```sh
omarchy plugin add https://gitea.com/tundeo/yolocam.git --enable
```

That clones into `~/.config/omarchy/plugins/yolocam`, enables the **service** (USB + control) and the **window**. It does not put a glyph on the bar.

Then build the helper:

```sh
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_DIR=/tmp/yolocam-target
cd ~/.config/omarchy/plugins/yolocam/helper
cargo build --release
cp -f /tmp/yolocam-target/release/yolocam ../bin/yolocam
omarchy restart shell
```

Keep Cargo’s `target/` **outside** the plugin tree. `omarchy plugin validate` rejects symlinks.

Optional: Super+Space app launcher.

```sh
bash ~/.config/omarchy/plugins/yolocam/scripts/write-desktop-entry.sh
```

Optional: a Hyprland key. Add one line to `~/.config/hypr/bindings.lua`. This plugin never writes that file:

```lua
o.bind("SUPER + SHIFT + C", "YoloCam", "omarchy-shell shell toggle yolocam '{}'")
```

## First run

Plug the camera with the original USB 3 **data** cable. Open **Apps → YoloCam** in the Omarchy menu, or:

```sh
omarchy-shell shell toggle yolocam '{}'
```

The window is unplugged (outline camera) until USB shows up, then fills, then goes accent when control is up. Click the picture for a 1280×720 preview. Click again (or leave Focus) to stop. Preview stops when the window closes.

If OBS, Meet, or Zoom already holds the UVC node, preview says busy and the rest of the window still works.

## Everyday use

Tabs: Video, Image, Focus, Zoom, Exposure, White balance, Audio, Status.

- **AF-S** — click the preview to pick a point
- **AF-C** — Large / Medium / Small, or Custom and drag a box
- **Face** — tracks a face

Hover ISO, shutter, zoom, and focus mode under the title for a one-line gloss.

With both an S7 and an S3 docked, pick the body under the title. A second body of the same model is ignored.

## Assistant

A skill teaches Grok (and other agents) to change ISO, zoom, and focus from speech or text. Copy it once:

```sh
mkdir -p ~/.grok/skills/yolocam ~/.agents/skills/yolocam
cp ~/.config/omarchy/plugins/yolocam/skills/yolocam/SKILL.md ~/.grok/skills/yolocam/
cp ~/.config/omarchy/plugins/yolocam/skills/yolocam/SKILL.md ~/.agents/skills/yolocam/
```

Then: “make the YoloCam brighter by 20%”, “zoom in a bit”, “switch to face detect”.

The agent uses `yolocam ctl` against the running watch. It must not run `yolocam get` / `set` / `status` while the window service is up.

## Update

```sh
omarchy plugin update yolocam
# rebuild the helper from helper/ if the Rust sources changed
omarchy restart shell
```

Until the restart, the shell keeps what it loaded at login.

## If something is wrong

If the menu does nothing after an update, restart the shell.

USB up but control down: the camera’s control process has stalled. Power-cycle data **and** power. `yolocam recover` is parked and does not work yet.

After a screen lock, preview may need a second click; a fully wedged UVC gadget still needs a power cycle.

Meet as a background Chromium tab is named Chromium, not Google Meet.

Logs and the control socket live under `$XDG_RUNTIME_DIR/yolocam/` (usually `/run/user/<uid>/yolocam/`).

## Remove

```sh
omarchy plugin remove yolocam
rm -f ~/.local/share/applications/yolocam.desktop
rm -rf ~/.grok/skills/yolocam ~/.agents/skills/yolocam
```

Then delete the `o.bind` line if you added one.

## Hardware

| | S7 | S3 |
|---|---|---|
| USB VID:PID | `46f8:0851` | `46f8:0855` |
| Gadget | CDC Ethernet | CDC NCM |
| Control | `192.168.129.10:12345` | `192.168.126.10:12345` |

Leave the USB gadget off the default route.

## Documentation

| | |
|---|---|
| [CLI](docs/cli.md) | Helper commands |
| [Protocol](docs/protocol.md) | Yunxi control channel |
| [Architecture](docs/architecture.md) | Plugin, helper, preview |
| [Troubleshooting](docs/troubleshooting.md) | Recover, UVC busy, mixed dock |
| [CONTRIBUTING](CONTRIBUTING.md) | Build the helper |

## License

[MIT](LICENSE)
