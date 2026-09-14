# YoloCam S3 live dump (2026-09-14)

Probed on this machine. Control is the same Yunxi protobuf as the S7.

| | S3 (this camera) | S7 (this host, earlier) |
|---|---|---|
| USB VID:PID | `46f8:0855` | `46f8:0851` |
| USB gadget | CDC NCM (`cdc_ncm`) | CDC Ethernet (`cdc_ether`) |
| Camera IP | `192.168.126.10` | `192.168.129.10` |
| Host IP | `192.168.126.11` | `192.168.129.11` |
| Control | TCP 12345, **same framing**, GET CID 1 resp 200 | same |
| Firmware | `YunxiCamera-ch131b-0.0.8` | `YunxiCamera-zhitong-ch431-0.0.9` |
| USB name | `S3: S3-26220290` (`/dev/video34`) | `YoloLiv YoloCam: Y-CAM-25360005` |
| USB serial | `S3-26220290` | `Y-CAM-25360005` |
| Protocol serial | `524A26220290` | `024825360005` |

Ping ~9–11 ms. video35 is UVC metadata — do not STREAMON it.

## UVC formats

MJPEG / H.264 / NV12 / YUY2. **No size below 1280×720.** Preview uses 1280×720 MJPEG, same as the S7.

| Size | MJPEG | H.264 | NV12 | YUYV |
|---|---|---|---|---|
| 1280×720 | 60/50/30/25 | 60/50/30/25 | 60/50/30/25 | 60/50/30/25 |
| 1920×1080 | 60/50/30/25 | 60/50/30/25 | 60/50/30/25 | 60/50/30/25 |
| 3840×2160 | 30/25 | 30/25 | — | — |

## GET 200 vs 100

Unsupported (resp 100) — hide in the panel:

- hue (11)
- gamma (12)
- zoom (13) — hide the Zoom tab; `zoom-speed` still 200
- lens-detail (63)
- mic-type (79)
- Audio tab — S3 has no line/mic jack (S7 does). Volume/denoise CIDs return 200 but are unused; the panel hides Audio for S3.

Probed extras (2026-09-14): CID 102 dynamic range GET 200 value 50; CID 105 WB tone GET 200 value 64; CID 111 manual focus GET 200 value 71 (0–100 slider). 16:9 / 9:16 is a **preview-only** rotate in the panel (camera physically turned); it is not a CID.

Present but fixed: aperture GET 200 value 0, aperture-range `{min:0,max:0}` — hide the aperture slider.

Surprise vs the plan: **distortion (26) and shading (27) are 200** on this S3. Do not hide them.

## Sample values at probe

ISO 100, shutter 16666 µs (1/60), focus-mode 4 (Face), brightness 1, sharpness 10, contrast/sat 50, WB auto 4150 K, HDMI auto, audio denoise middle, AGC on.
