# Control protocol

YoloCam S7 and S3 are composite USB gadgets: a CDC network interface for control, and a UVC interface for video. Control is a dual-envelope TCP stream wrapping `tv.yunxi.protobuf.Message` on port **12345**.

This document describes the subset this plugin speaks. Settings written with SET persist on the camera.

## Hardware

| | S7 | S3 |
|---|---|---|
| USB VID:PID | `46f8:0851` | `46f8:0855` |
| Network class | CDC Ethernet (`cdc_ether`) | CDC NCM (`cdc_ncm`) |
| Camera IPv4 | `192.168.129.10` | `192.168.126.10` |
| Host IPv4 (DHCP) | `192.168.129.11` | `192.168.126.11` |
| Control | TCP 12345 | TCP 12345 (same framing) |
| Video | UVC, no size below 1280×720 | same |

Userspace does not care ECM versus NCM after DHCP: both are a `net/` interface. Pair the UVC node by walking sysfs to the USB parent. Do not pick “the first video device whose name contains yolocam” — a mixed dock would attach the wrong preview.

A second S7 or second S3 is ignored. Two bodies of the same model both claim the same `.10` address; this plugin does not bind sockets to a device.

On the gadget network the camera also offers telnet (23) and FTP (21). Credentials and recovery are in [troubleshooting](troubleshooting.md).

## Wire format

Each frame:

```
OUTER  8 bytes   0x08, client_type, 0x00, 0x00, inner_size as little-endian u32
INNER  6 bytes   0xA5, proto_size as big-endian u32, XOR of those five bytes
PAYLOAD          serialized protobuf Message
TRAILER 2 bytes  XOR of payload, 0x5A
```

`client_type` is `0x01` host → camera and `0x00` camera → host. `inner_size` is `proto_size + 8`. Frames larger than 64 KiB are rejected.

## Message

| Field | Role |
|---|---|
| `type` | 1 request (GET), 2 set, 3 post, 4 response, 5 notify |
| `seq` | Host sequence; echoed on a well-behaved response |
| `timeout` | Milliseconds, typically 3000 |
| `resp_code` | 200 success, 100 unsupported or out of range |
| `id` | Command ID (CID) |
| `body` | Integer, string, version, range, WB offset, lens blob, or rect list |

Some responses echo the CID and omit `seq`. The helper treats a response as the in-flight RPC if the sequence matches **or** the CID matches. Only one RPC is in flight per camera.

## Heartbeat and pacing

`live_camera_app` stops handling commands if the host fires on the order of 25 unpaced GETs, or if a second TCP client opens 12345 while another already holds it.

This helper:

- Holds one TCP client per plugged camera
- Spaces RPCs by **160 ms**
- Idle-fills one CID at a time (hero parameters first)
- Waits **400 ms** for a GET reply and **2.5 s** for an Int/WB SET (HDMI format retrain is slow)
- Sends CID **215** (stream heartbeat) about every 25 s
- Does not restart the camera because a GET timed out
- Does not SET a CID that has no named parameter

If 12345 is refused, recover the control process rather than retrying GET-all. See [troubleshooting](troubleshooting.md).

## Named parameters

SET is only allowed for names in this table. The JSON wire uses the **name** column. Kind is both the GET body and the only SET body that name accepts.

`GetOnly` cannot be SET. `WbOffset` requires `r` and `b`, not an integer. Unknown CIDs are treated as get-only.

| Name | CID | Group | Kind |
|---|---|---|---|
| firmware | 1 | status | GetOnly (version) |
| frame-rate | 5 | video | enum 25 / 30 / 50 / 60 |
| resolution | 6 | video | enum auto / 4k / 1080p / 720p |
| brightness | 7 | image | int 0–100 |
| sharpness | 8 | image | int −10–30 (wire: negatives as 61–70) |
| contrast | 9 | image | int 0–100 |
| saturation | 10 | image | int 0–100 |
| hue | 11 | image | int 0–100 |
| gamma | 12 | image | int 0–100 |
| image-version | 4 | image | parked; not in the panel. Compose V0/V1 colour profile (CID_SCENE). GET is not sent. |
| zoom | 13 | zoom | int 50–98 (`mult = 0.0625 × raw − 2.125`). S7 optical via CID 13 Int. S3 4× digital ePTZ: SET is a centred crop rect (`w=h=1/mult`); the panel also scales the preview. |
| exposure-mode | 14 | exposure | enum auto / manual |
| ev | 15 | exposure | int 0–16 (8 is 0 EV) |
| anti-flicker | 16 | exposure | enum off / 50hz / 60hz |
| iso | 17 | exposure | int 100–12800 |
| shutter-us | 18 | exposure | microseconds, 125–40000 |
| aperture | 20 | exposure | int 0–160 |
| wb-mode | 21 | wb | enum manual / auto |
| wb-temp | 22 | wb | Kelvin 2000–10000 |
| focus-mode | 23 | focus | enum afs=1 / manual=2 / afc=3 / face=4 |
| distortion | 26 | image | enum off / on |
| shading | 27 | image | enum off / on |
| aperture-range | 31 | status | GetOnly |
| shutter-range | 32 | status | GetOnly |
| wb-offset | 33 | wb | `{r, b}` |
| focus-box | 38 | focus | enum large / medium / small |
| caf | 42 | focus | enum off / on (not exposed; firmware ignores) |
| focus-speed | 47 | focus | enum fast=1 / medium=3 / slow=5 |
| lens-status | 48 | status | GetOnly |
| usb-version | 60 | status | GetOnly |
| hdmi-format | 61 | video | HDMI port size: enum auto / 4k / 1080p / 720p (SET can take ~1 s). Independent of `resolution` (USB/UVC, CID 6). |
| lens-detail | 63 | status | GetOnly |
| hdmi-color | 65 | video | enum auto / rgb / yuv422 |
| audio-volume | 73 | audio | int 0–100 |
| audio-denoise | 75 | audio | enum off / weak / middle / strong |
| audio-reverb | 76 | audio | enum off / weak / middle / strong (not exposed) |
| audio-delay | 77 | audio | int 0–400 ms |
| audio-agc | 78 | audio | enum off / on |
| mic-type | 79 | audio | enum line / mic |
| clut | 94 | image | enum off / on |
| clut-mix | 96 | image | int 0–255 |
| hdr | 98 | exposure | enum off / on |
| dynamic-range | 102 | image | int 0–100 |
| wb-lock | 104 | wb | enum off / on |
| wb-tone | 105 | wb | int 0–128 |
| exposure-lock | 106 | exposure | enum off / on |
| manu-focus | 111 | focus | int 0–100 |
| zoom-speed | 112 | zoom | int 1–5 (not exposed; firmware ignores) |

### Shutter denominators

| 1/N | µs | 1/N | µs |
|---|---|---|---|
| 8000 | 125 | 500 | 2000 |
| 6400 | 156 | 400 | 2500 |
| 5000 | 200 | 320 | 3125 |
| 4000 | 250 | 250 | 4000 |
| 3200 | 312 | 200 | 5000 |
| 2500 | 400 | 160 | 6250 |
| 2000 | 500 | 125 | 8000 |
| 1600 | 625 | 100 | 10000 |
| 1250 | 800 | 80 | 12500 |
| 1000 | 1000 | 60 | 16666 |
| 800 | 1250 | 50 | 20000 |
| 640 | 1562 | 30 | 33333 |
| | | 25 | 40000 |

## S3 capabilities

The S3 reports `resp_code` 100 (or is unused) for several S7 controls. The helper emits a `caps` event so the panel hides those rows. The current hide list:

hue, gamma, lens-detail, mic-type, audio volume/denoise/delay/AGC, HDMI format/colour, brightness, distortion, CLUT / LUT mix.

The Audio tab is hidden on S3 (no line/mic jack). Zoom is on both bodies: S7 optical (CID 13 Int), S3 4× digital ePTZ (CID 13 Rect crop plus preview scale). Image version is parked. 16:9 / 9:16 in the panel is a **preview-only** rotate; it is not a CID. Orientation must not be written as a SET.

Aperture on a body without a driven iris is present as GET 200 with a degenerate range; the panel hides the slider when the range is empty.

## Autofocus overlays

| CID | Role |
|---|---|
| 59 | Face boxes (NOTIFY while the socket is open). The helper keeps the most face-like rectangle |
| 24 | AF-S point and AF-C custom box (SET rect) |
| 34 | AF area notify |
| 38 | AF-C Large / Medium / Small |

Rects are normalized 0–1 in the frame. Overlay is drawn on the Focus tab only.

## Video

Preview uses **1280×720 MJPEG** at about 15 fps. NV12 and YUY2 are not used for the panel viewfinder. UVC is exclusive: one STREAMON. Click-to-start in the panel so preview never fights OBS unless asked.

## What this plugin will not SET

Command IDs that are not in the named table cannot be SET. In particular CID 29, 55, and 58 are not named here: writing them has rebooted an S7. Do not add a SET for an undocumented GET-200 integer without a hardware-safe probe that is not a SET of that CID on the S7.
