# Architecture

One Omarchy plugin, one helper process, at most two camera sessions.

```
Omarchy shell
  Service.qml  ── spawns ──►  bin/yolocam watch
  Panel.qml    ── JSON  ──►  stdin / $XDG_RUNTIME_DIR/yolocam/ctl.sock
       ▲                         │
       └── JSON-lines events ────┘
                                 │
                    ┌────────────┴────────────┐
                    ▼                         ▼
              Session S7                 Session S3
           192.168.129.10:12345       192.168.126.10:12345
           preview …/<s7-serial>/     preview …/<s3-serial>/
```

## Plugin layout

| Path | Role |
|---|---|
| `manifest.json` | Omarchy plugin id `yolocam`, bar widget + service |
| `Service.qml` | Long-lived `Process` running `watch`; applies events into panel state |
| `Panel.qml` | Bar glyph and settings panel |
| `Model.js` | Shared labels, zoom/EV/shutter formatting |
| `bin/yolocam` | Release helper |
| `helper/` | Rust crate (MIT) |

The shell hot-reloads QML under `~/.config/omarchy/plugins/` on save. Replacing `bin/yolocam` takes effect when watch restarts (the service relaunches it after exit).

## Watch

`yolocam watch` polls, in one thread:

- camera TCP (S7 and/or S3)
- Unix `ctl.sock`
- stdin
- inotify on `/dev` and USB/net/video sysfs (with a timer fallback)

Each session has its own TCP client, fill cursor, heartbeat, and optional preview. Commands without `model` / `id` go to the **selected** camera (S7 if both just appeared).

RPC rules per session:

- At most one in-flight GET or SET
- 160 ms pace
- A queued SET replaces the previous queued SET (last slider value wins)
- GET timeout is silent; SET timeout of an int or WB offset is an error event
- Preview pauses idle GETs; Refresh is urgent and tab-scoped so it cannot hang behind STREAMON

JSON commands are parsed into `Cmd`. Named SET goes through `ParamId` and `parse_set`, so the body type must match the GET kind. Caps (`unsupported`) are `ParamId` values emitted as the same strings the panel already uses.

## Preview

UVC MJPEG is written as ping-pong JPEGs under `$XDG_RUNTIME_DIR/yolocam/<serial>/` (`preview-a.jpg` / `preview-b.jpg` plus a `slot` file). Quickshell `Image` loads a file URL; the helper decodes luma only to decide “lens covered.”

`Preview` owns the capture fd. Dropping it closes the fd so a blocking `STREAMON`/`DQBUF` in the worker returns instead of wedging watch. `EBUSY` is reported with the process that holds the node (OBS, Meet, Zoom when those can be named).

Only one preview runs. Changing selection stops the other camera’s viewfinder.

## Discovery

Walk `/sys/bus/usb/devices` for vendor `46f8` and product `0851` (S7) or `0855` (S3). Keep the first serial of each. For each slot, find `net/` (iface, host IPv4, camera = sibling `.10`) and `video4linux` by USB parent.

Glyph code is internal inventory: 0 none, 3 S3, 7 S7, 10 both. The bar draws colour, not those digits.

## Helper modules

| Module | Responsibility |
|---|---|
| `wire` | Dual-envelope framing |
| `pb` | Minimal protobuf codec for `Message` |
| `params` | `ParamId`, kinds, encode/decode |
| `cmd` | JSON command enum |
| `client` | Non-blocking TCP + 160 ms pace |
| `discover` | USB / net / UVC inventory |
| `watch` | Event loop (binary) |
| `preview` | V4L2 MJPEG |
| `recover` | Telnet restart of `live_camera_app` |
| `holders` | Who has the UVC node open |

Build with `CARGO_TARGET_DIR` outside the plugin tree. Clippy: `deny(clippy::all)`, `warn(clippy::pedantic)`.
