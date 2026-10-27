# Configuration

The user-facing explanation is in [README.md](../README.md). This file is the contract: what lives **inside** the plugin tree, what you add **outside** it, and what the camera itself stores.

CamY does not ship a `~/.config/camy/` directory. Camera parameters persist on the body. Window chrome persists in Hyprland and Omarchy.

## Inside the plugin

Cloned to `~/.config/omarchy/plugins/io.github.t-zero-dev.camy`. `omarchy plugin validate` refuses any symlink in this folder, including Cargo `target/`.

| Path | Role |
| --- | --- |
| `manifest.json` | Plugin id `io.github.t-zero-dev.camy`, kinds `service` + `panel` |
| `Service.qml` | Long-lived `camy watch`; IPC target `camy` |
| `Panel.qml` | FloatingWindow title `CamY`, implicit size 720×1220, minimum 720×720 |
| `Model.js` | Tab ids, zoom/EV/shutter labels |
| `bin/camy` | Checkout helper (gitignored; `scripts/build-helper.sh`) |
| `helper/` | Rust sources (crate name `camy`) |
| `helper/release.pin` | Pinned GitHub Release tag, asset names, sha256 |
| `run.sh` | `--ensure` / `--print-path` bootstrap |
| `scripts/fetch-helper.sh` | Download, hash, install under `$XDG_DATA_HOME/camy/bin` |
| `scripts/write-desktop-entry.sh` | Apps launcher entry (run from `run.sh --ensure`) |
| `scripts/install-window-rules.sh` | Copy `hypr/camy.lua` into `~/.config/hypr/` and require it |
| `hypr/camy.lua` | Float, centre, size, Super+T recentre |
| `scripts/build-helper.sh` | Checkout helper into user cache, then `bin/` |
| `skills/camy/SKILL.md` | Source copy of the agent skill |
| `docs/` | This tree |

`Panel.qml` sizes the window. Hyprland’s size rule applies on map; the panel also float/resize/centre on open.

### Plugin settings object

Omarchy injects the `plugins[]` entry for `io.github.t-zero-dev.camy` as `settings` on both Service and Panel.

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `preview` | bool | `true` | When `false`, click and Space do not start UVC |

No other keys are read. ISO, shutter, tab, selected camera, and preview-on are session state, not this object.

## Outside the plugin

### Enable — `~/.config/omarchy/shell.json`

```json
"plugins": [
  { "id": "io.github.t-zero-dev.camy" }
]
```

Kinds are service and panel, so enablement is this list. `omarchy plugin add … --enable` writes it.

To disable preview from config:

```json
{ "id": "io.github.t-zero-dev.camy", "preview": false }
```

### Window — `~/.config/hypr/camy.lua`

`run.sh --ensure` copies [hypr/camy.lua](../hypr/camy.lua) to `~/.config/hypr/camy.lua` and appends a `pcall(require, "hypr.camy")` block to `hyprland.lua` if it is missing. Title match `^CamY`, class `org.quickshell`. Do not tag CamY `+floating-window`; that size is 875×600 and would override 720×1220.

| Rule | Why |
| --- | --- |
| `float` | Stays a card window. |
| `center` | Opens in the middle of the monitor, below the bar reserved area. |
| `size` | Width 720. Height 1220 on a tall dock, 90% of logical `monitor_h` on a short laptop. |
| `suppress_event` | Fullscreen and maximize are ignored. Resize by edges remains. |
| Super+T | Other windows still toggle tile. CamY only recentres. |

The panel also float/resize/centre on open so the first frame is right before Hyprland reloads.

### Keybind — `~/.config/hypr/bindings.lua`

Optional. The plugin never writes this file.

```lua
o.bind("SUPER + SHIFT + C", "CamY", "omarchy-shell shell toggle io.github.t-zero-dev.camy '{}'")
```

### Launcher — `~/.local/share/applications/camy.desktop`

Written by `scripts/write-desktop-entry.sh`. Exec is `omarchy-shell shell toggle io.github.t-zero-dev.camy '{}'`. `StartupNotify=false` and `DBusActivatable=false` so the launcher does not wait on a new app-id (the window is class `org.quickshell`).

### Agent skill

| Copy from | Copy to |
| --- | --- |
| `plugins/io.github.t-zero-dev.camy/skills/camy/SKILL.md` | `~/.grok/skills/camy/SKILL.md` |
| same | `~/.agents/skills/camy/SKILL.md` |

The plugin does not install these copies itself.

### USB gadget routing

The camera’s CDC interface is a `/24` used only for TCP 12345. If NetworkManager gives that NIC a default route, mark it never-default. This is host network config, not a plugin file.

## Runtime (not config)

`$XDG_RUNTIME_DIR/camy/` (usually `/run/user/<uid>/camy/`):

| File | Role |
| --- | --- |
| `ctl.sock` | Watch control socket (`camy ctl`) |
| `<serial>/preview-a.jpg`, `preview-b.jpg` | Ping-pong MJPEG for the panel |
| `<serial>/slot` | Which JPEG the panel should load |

| `bootstrap.log` | Why fetch/install of the pinned helper failed |

These vanish at logout. They are not preferences.

### Installed helper — `~/.local/share/camy/`

The pinned helper lives outside the plugin tree:

| Path | Role |
| --- | --- |
| `$XDG_DATA_HOME/camy/bin/camy` | Fetched, hash-checked helper (`~/.local/share/camy/bin/camy`) |
| `$XDG_CACHE_HOME/camy/` | Fetch work dir and cargo target for checkout builds |

## Stored on the camera

SET of a named parameter persists on the body (ISO, white balance, HDMI format, audio, …). Preview orientation, preview flip, and AF box drawn in the panel are session UI except where the helper also writes `af-target` / `af-area`.

## Build environment

| Variable | Value |
| --- | --- |
| `CARGO_TARGET_DIR` | `${XDG_CACHE_HOME:-$HOME/.cache}/camy/target` (mode 700, via `scripts/build-helper.sh`) |
| `PATH` | include `~/.cargo/bin` |

## What the plugin will not write

- `~/.config/hypr/bindings.lua`
- `~/.config/omarchy/shell.json` except through `omarchy plugin add/remove`
- `~/.config/camy/`

`run.sh --ensure` does write `~/.config/hypr/camy.lua` and, if missing, a `camy-window-rules` require block at the end of `hyprland.lua`.
