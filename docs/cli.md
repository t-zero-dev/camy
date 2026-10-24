# Helper CLI

`bin/camy` talks to a docked S7 or S3 over the Yunxi control port. The CamY service runs `camy watch` and **owns** TCP 12345. One-shot commands that open their own socket (`get`, `set`, `probe`, `get-all`, firmware `status`) fail or wedge the camera if watch is already connected.

Use `camy ctl` or `omarchy-shell camy …` against a running watch. Close the window / stop the service before a second TCP client.

```bash
PLUGIN=~/.config/omarchy/plugins/io.github.t-zero-dev.camy
BIN="$PLUGIN/bin/camy"
$BIN help
```

## Shell IPC

The service registers Quickshell IPC target `camy`. These talk to watch through the panel service, not a second TCP client.

```bash
omarchy-shell camy state
omarchy-shell camy iso 400
omarchy-shell camy shutter 16666
omarchy-shell camy zoom 66
omarchy-shell camy focusMode 4
omarchy-shell camy refresh
```

`state` returns JSON: `usb`, `control`, `glyph`, `selected`, `cameras`, `serial`, `firmware`, `preview`, `previewBusy`, `previewHolder`, `lensCovered`, `applyStatus`, `error`, `values`.

`scene` is registered and returns `unsupported`.

Open or hide the window:

```bash
omarchy-shell shell toggle io.github.t-zero-dev.camy '{}'
omarchy-shell shell summon io.github.t-zero-dev.camy '{}'
omarchy-shell shell hide io.github.t-zero-dev.camy
```

## Commands

### ctl

Forward a JSON command to a running watch (`$XDG_RUNTIME_DIR/camy/ctl.sock`). This is the daily CLI while the plugin is enabled.

```bash
$BIN ctl set iso 400
$BIN ctl set focus-mode 4
$BIN ctl '{"cmd":"set","name":"iso","value":"400"}'
$BIN ctl '{"cmd":"preview","on":true}'
$BIN ctl '{"cmd":"select","model":"s3"}'
$BIN ctl '{"cmd":"refresh"}'
```

If the socket is missing, the error is `watch not running`. Enable the plugin / restart the shell. Do not open a second TCP client.

### status

USB, gadget NIC, control reachability, firmware (only if watch is **not** holding the port), and an inventory of plugged cameras.

```bash
$BIN status
```

When `ctl.sock` exists, `status` skips the firmware GET so it does not steal 12345. Use `omarchy-shell camy state` for live values.

### probe

Version handshake (GET CID 1). Requires that watch is not running.

### list

Named parameters this helper knows: name, command ID, group, whether SET is allowed, and value kind.

### get / get-all / set

Own TCP 12345. Use only when watch is down.

```bash
$BIN get iso shutter-us focus-mode
$BIN set iso 400
$BIN set focus-mode face
$BIN iso 400
$BIN shutter 1/60          # or microseconds: 16666
$BIN wb-temp 5500
$BIN aperture 80
```

White-balance offset is not a raw integer. The panel sends `r` and `b` through watch; the CLI `set` path rejects a bare number.

### watch

JSON-lines events on stdout; commands on stdin (and on `ctl.sock`). This is what the Omarchy service runs. See [architecture.md](architecture.md).

### preview

Foreground MJPEG viewfinder for debugging. The panel uses `watch` for preview.

```bash
$BIN preview start    # until stdin closes
$BIN preview stop
```

### recover

Parked. The binary still has the subcommand; the product must not use it. USB present and control refused: power-cycle data and power.

## Watch JSON commands

The panel sends one JSON object per line. `model` is `s3` or `s7`; omitted commands hit the selected camera.

| `cmd` | Fields | Effect |
| --- | --- | --- |
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
