# YoloCam S7 live dump (2026-08-29)

Probed on this machine. Control is the Yunxi protobuf protocol documented for the S3, with a different USB ID and gadget subnet.

| | S3 (published) | S7 (this camera) |
|---|---|---|
| USB VID:PID | `46f8:0855` | `46f8:0851` |
| USB gadget | CDC NCM | CDC Ethernet (`cdc_ether`) |
| Camera IP | `192.168.126.10` | `192.168.129.10` |
| Host IP | `192.168.126.11` | `192.168.129.11` |
| Control | TCP 12345 | TCP 12345 (open, same framing) |
| Firmware | YunxiCamera-ch131b-0.0.6 | `YunxiCamera-zhitong-ch431-0.0.9` |
| UVC name | | `YoloLiv YoloCam: Y-CAM-25360005` (`/dev/video34`) |
| Serial | | USB `Y-CAM-25360005`, protocol `024825360005` |

## UVC formats

MJPEG / H.264 / NV12 / YUY2 at 1280×720, 1920×1080, 3840×2160. **No sub-720p MJPEG.** Preview uses 1280×720 MJPEG and caps display at ~15 fps. NV12/YUY2 are not used for preview.

## GET 200 (int unless noted)

frame-rate 30, resolution 1, brightness 50, sharpness 5, contrast 50, saturation 50, hue 50, gamma 0, distortion 1 (CID 26), shading/vignette 1 (CID 27), zoom 50 (1.0×), exposure-mode 0 (auto), ev 8, anti-flicker 1 (50 Hz), iso 100, shutter-us 16384, aperture 3, wb-mode 1 (auto), wb-temp 4150, focus-mode 4 (face), focus-box 0, caf 1, focus-speed 1, lens-status 1, usb-version 3, hdmi-format 1 (4K), hdmi-color 0, audio-volume 50, denoise 0, delay 0, agc 1, mic-type 1, clut 1, clut-mix 1.

Range aperture 14–160, shutter 29–40000 µs. WB offset r/b = 10.

Lens detail (CID 63) strings include `0_4152` and `1.44` / `16.31` (mounted lens).

Mapped S7 extras in the image tab: CID 26 `CID_ENABLE_DISTORTION`, CID 27 `CID_ENABLE_SHADING` (vignette). Other 200s not in the panel: CID 25, 29, 40, 41, 49, 50, 54, 55, 58, 62, 64.

## Face / AF boxes

CID 59 NOTIFY floods while the control socket is open. The helper keeps the single most face-like box. Overlay is Focus-tab only, `Color.accent`: Face uses CID 59. AF-S and AF-C write CID 24 (`FOCUS_TARGET_LOCATION`); CID 34 is notify-only. AF-S uses a fixed large point (~10% of the frame). AF-C Large/Medium/Small also SET CID 38; Custom is a drawn CID 24 rect. Clicks on the Focus-tab preview do not toggle the viewfinder. A thicker accent border on the preview pane is the live-tally chrome.

## Control lockup

The camera's `live_camera_app` dies if you fire ~25+ GETs with no pacing, or if a second client opens TCP 12345 while watch already holds it. Watch paces every RPC by 160 ms and idle-fills one CID at a time so the event loop can still start preview. Do not auto-restart the camera from GET timeouts — that loop is what wedged the panel. If 12345 is refused, run `yolocam recover` (telnet `root` / `yunxi.tv`, restart `/app/bin/live_camera_app`). Never run `yolocam status` while the bar helper is connected.

## Preview rule

`uvcvideo` is a single streamer. If OBS holds `/dev/video34`, preview returns `busy` and the panel keeps every Compose control.
