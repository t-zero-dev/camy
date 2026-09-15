# Helper CLI

`bin/yolocam` talks to a docked S7 or S3 over the Yunxi control port. The bar widget runs `yolocam watch` and **owns** TCP 12345. One-shot commands that open their own socket (`status` firmware probe, `get`, `set`, `probe`, `get-all`) fail or wedge the camera if watch is already connected.

Use `yolocam ctl` to send JSON to a running watch, or close the panel / stop the widget first.

```bash
BIN=~/.config/omarchy/plugins/yolocam/bin/yolocam
$BIN help
```

## Commands

### status

USB, gadget NIC, control reachability, firmware (if watch is not holding the port), and an inventory of plugged cameras.

```bash
$BIN status
```

### probe

Version handshake (GET CID 1). Requires that watch is not running.

### list

Named parameters this helper knows: name, command ID, group, whether SET is allowed, and value kind.

### get

Read one or more named parameters.

```bash
$BIN get iso shutter-us focus-mode
```

### get-all

Read every named parameter, paced. Prefer the panel Refresh action while watch is running.

### set

Write a named integer or enum parameter.

```bash
$BIN set iso 400
$BIN set focus-mode face
$BIN set distortion on
```

White-balance offset is not a raw integer. The panel sends `r` and `b` through watch; the CLI `set` path rejects a bare number.

### iso / shutter / wb-temp / aperture

Aliases for `set`.

```bash
$BIN iso 400
$BIN shutter 1/60          # or microseconds: 16666
$BIN wb-temp 5500
$BIN aperture 80
```

### watch

JSON-lines events on stdout; commands on stdin (and on `$XDG_RUNTIME_DIR/yolocam/ctl.sock`). This is what the Omarchy service runs. See [architecture](architecture.md).

### ctl

Forward a JSON command to a running watch.

```bash
$BIN ctl '{"cmd":"set","name":"iso","value":"400"}'
$BIN ctl set iso 400
$BIN ctl '{"cmd":"preview","on":true}'
```

### recover

Telnet to the camera, restart `/app/bin/live_camera_app`, and wait until port 12345 accepts. Use this when USB is present but control is refused — not a power-cycle.

```bash
$BIN recover
```

### preview

Foreground MJPEG viewfinder for debugging. The panel uses `watch` for preview.

```bash
$BIN preview start    # until stdin closes
$BIN preview stop
```

## Watch JSON commands

The panel sends one JSON object per line. `model` is `s3` or `s7`; omitted commands hit the selected camera.

| `cmd` | Fields | Effect |
|---|---|---|
| `select` | `model` or `id` | Switch the selected session |
| `tab` | `name` | Reorder idle fill toward that group |
| `get-all` | | Restart warm then remaining GETs |
| `refresh` | | GET the open tab only |
| `set` | `name`, `value` | Write a named parameter |
| `set` | `name: wb-offset`, `r`, `b` | Write white-balance offset |
| `af-target` / `af-area` | `x`, `y`, `w`, `h` | Normalized AF rectangle |
| `preview` | `on` | Start or stop the viewfinder |
| `heartbeat` | | Queue CID 215 |
| `reconnect` | | Drop and reopen TCP |

Unknown `cmd` or `name` values are rejected. Named SET of a read-only parameter, or an integer for `wb-offset`, is rejected. That gate is what keeps undocumented command IDs off the wire.

## Output

Successful one-shot commands print a JSON object with `"ok": true`. Failures print `"ok": false` and `"error"` on stderr and exit 1.
