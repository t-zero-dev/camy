# Configuration

The user-facing explanation is in [README.md](../README.md). This file is the contract: what lives **inside** the plugin tree, what you add **outside** it, and what the camera itself stores.

Omacam does not ship a `~/.config/omacam/` directory. Camera parameters persist on the body. Window chrome persists in Hyprland and Omarchy.

## Inside the plugin

Cloned to `~/.config/omarchy/plugins/io.github.t-zero-dev.omacam`. `omarchy plugin validate` refuses any symlink in this folder, including Cargo `target/`.

| Path | Role |
| --- | --- |
| `manifest.json` | Plugin id `io.github.t-zero-dev.omacam`, kinds `service` + `panel` |
| `Service.qml` | Long-lived `yolocam watch`; IPC target `omacam` |
| `Panel.qml` | FloatingWindow title `Omacam`, implicit size 720×1220, minimum 720×720 |
| `Model.js` | Tab ids, zoom/EV/shutter labels |
| `bin/yolocam` | Release helper (gitignored; you build it) |
| `helper/` | Rust sources (crate name `yolocam`) |
| `scripts/write-desktop-entry.sh` | Optional launcher entry |
| `skills/yolocam/SKILL.md` | Source copy of the agent skill |
| `docs/` | This tree |

`Panel.qml` sizes the window. Hyprland’s size rule, if present, applies on map and wins for the first frame.

### Plugin settings object

Omarchy injects the `plugins[]` entry for `io.github.t-zero-dev.omacam` as `settings` on both Service and Panel.

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `preview` | bool | `true` | When `false`, click and Space do not start UVC |

No other keys are read. ISO, shutter, tab, selected camera, and preview-on are session state, not this object.

## Outside the plugin

### Enable — `~/.config/omarchy/shell.json`

```json
"plugins": [
  { "id": "io.github.t-zero-dev.omacam" }
]
```

Kinds are service and panel, so enablement is this list. `omarchy plugin add … --enable` writes it.

To disable preview from config:

```json
{ "id": "io.github.t-zero-dev.omacam", "preview": false }
```

### Window — `~/.config/hypr/hyprland.lua`

The plugin never writes Hyprland. Recommended rules, title match `^Omacam`, class `org.quickshell`:

```lua
o.window({ title = "^Omacam" }, { float = true })
o.window({ title = "^Omacam" }, { center = true })
o.window({ title = "^Omacam" }, { size = "720 min(1220, monitor_h*0.90)" })
o.window({ title = "^Omacam" }, { suppress_event = "fullscreen maximize" })
```

| Rule | Why |
| --- | --- |
| `float` | Stays a card window. Tag `+floating-window` forces 875×600 and overrides size. |
| `center` | Opens in the middle of the monitor, below the bar reserved area. |
| `size` | Width 720. Height 1220 on a tall dock, 90% of logical `monitor_h` on a short laptop. Expressions must not contain spaces inside `min(…)`. |
| `suppress_event` | Fullscreen and maximize are ignored. Resize by edges remains. |

Size is a static map rule. `hyprctl reload`, then close and reopen Omacam.

Optional: Super+T on this window recentres and keeps float, while other windows still toggle tile. That is a local override of Omarchy’s Super+T, not a plugin file.

```lua
hl.unbind("SUPER + T")
o.bind("SUPER + T", "Toggle window floating/tiling", function()
  local w = hl.get_active_window()
  local title = w and tostring(w.title or "") or ""
  if title:find("^Omacam") then
    hl.dispatch(hl.dsp.window.float({ action = "set", window = w }))
    hl.dispatch(hl.dsp.window.center({ window = w }))
    return
  end
  hl.dispatch(hl.dsp.window.float({ action = "toggle" }))
end)
```

### Keybind — `~/.config/hypr/bindings.lua`

Optional. The plugin never writes this file.

```lua
o.bind("SUPER + SHIFT + C", "Omacam", "omarchy-shell shell toggle io.github.t-zero-dev.omacam '{}'")
```

### Launcher — `~/.local/share/applications/omacam.desktop`

Written by `scripts/write-desktop-entry.sh`. Exec is `omarchy-shell shell toggle io.github.t-zero-dev.omacam '{}'`. `StartupNotify=false` and `DBusActivatable=false` so the launcher does not wait on a new app-id (the window is class `org.quickshell`).

### Agent skill

| Copy from | Copy to |
| --- | --- |
| `plugins/io.github.t-zero-dev.omacam/skills/yolocam/SKILL.md` | `~/.grok/skills/omacam/SKILL.md` |
| same | `~/.agents/skills/omacam/SKILL.md` |

The plugin does not install these copies itself.

### USB gadget routing

The camera’s CDC interface is a `/24` used only for TCP 12345. If NetworkManager gives that NIC a default route, mark it never-default. This is host network config, not a plugin file.

## Runtime (not config)

`$XDG_RUNTIME_DIR/yolocam/` (usually `/run/user/<uid>/yolocam/`):

| File | Role |
| --- | --- |
| `ctl.sock` | Watch control socket (`yolocam ctl`) |
| `<serial>/preview-a.jpg`, `preview-b.jpg` | Ping-pong MJPEG for the panel |
| `<serial>/slot` | Which JPEG the panel should load |

These vanish at logout. They are not preferences. The directory is still named `yolocam` because the helper crate is.

## Stored on the camera

SET of a named parameter persists on the body (ISO, white balance, HDMI format, audio, …). Preview orientation, preview flip, and AF box drawn in the panel are session UI except where the helper also writes `af-target` / `af-area`.

## Build environment

| Variable | Value |
| --- | --- |
| `CARGO_TARGET_DIR` | `/tmp/omacam-target` (or any path **outside** the plugin folder) |
| `PATH` | include `~/.cargo/bin` |

## What the plugin will not write

- `~/.config/hypr/bindings.lua`
- `~/.config/hypr/hyprland.lua`
- `~/.config/omarchy/shell.json` except through `omarchy plugin add/remove`
- `~/.config/omacam/`
