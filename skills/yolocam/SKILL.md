---
name: omacam
description: >
  Configure a YoloLiv YoloCam S7 or S3 (ISO, shutter, zoom, focus mode, exposure)
  from natural language via the unofficial Omacam Omarchy plugin. Use when the
  user says omacam, yolocam, YoloCam, camera brighter, darker, zoom in, zoom out,
  autofocus, AF-C, AF-S, face detect, or asks to change camera exposure. Use when
  they run /omacam or /yolocam.
---

# Omacam

Talk to the **running watch** only. The Omarchy service owns TCP 12345.

```bash
PLUGIN="$HOME/.config/omarchy/plugins/io.github.t-zero-dev.omacam"
BIN="$PLUGIN/bin/yolocam"
```

## Do

- Read current values: `omarchy-shell omacam state` (JSON: `usb`, `control`, `values`)
- Change a parameter: `$BIN ctl set <name> <value>`
- Open the window: `omarchy-shell shell toggle io.github.t-zero-dev.omacam '{}'`

If `ctl` says `watch not running`, the plugin service is not enabled. Tell the user to enable Omacam; do not open a second TCP client.

## Do not

- `$BIN get`, `set`, `probe`, `get-all`, or `status` while watch is up (second 12345 client; can lock the camera)
- `$BIN recover` (parked; does not work)
- Guess ISO or shutter numbers outside the camera range

## Parameters

| Name | What | Range / values |
| --- | --- | --- |
| `iso` | Sensor gain | 100–12800 |
| `shutter-us` | Exposure time in microseconds | 125–40000 (16666 is 1/60 s) |
| `zoom` | Optical zoom raw | 50–98 (1.0×–4.0×) |
| `focus-mode` | Autofocus | `1` AF-S, `3` AF-C, `4` Face |
| `ev` | Exposure compensation | camera enum via `ctl set ev <n>` |

## Relative requests

1. Run `omarchy-shell omacam state` and parse `values`.
2. If `usb` is false, say the camera is unplugged.
3. If `control` is false, say USB is up but control is down; do not recover.
4. Compute the new value, clamp to the range, then `ctl set`.

**Brighter / darker by X%:** `iso_new = clamp(round(iso * (1 ± X/100)), 100, 12800)`.

**Zoom in / out by X%:** raw 50 = 1.0×, 98 = 4.0×. `zoom_new = clamp(round(zoom ± (98-50)*X/100), 50, 98)`. “Zoom in a bit” is +15% of the raw span (~7). “Zoom to 2×” is raw `round((2 + 2.125) / 0.0625)` = 66.

**Shutter 1/N:** `shutter-us = round(1000000 / N)`.

**Focus:** “face” → 4, “continuous” / “AF-C” → 3, “single” / “AF-S” / “click to focus” → 1.

After SET, one short confirmation of the new value. Do not spam ctl.
