# Two-camera plan: one S7, one S3, or both

Status: S3 spike complete on this host (`ch131b-0.0.8`, same framing). Caps hide hue/gamma/zoom/mic-type/lens-detail. Mixed dock still wants an S7 beside the S3.

**Supported deployments (maximum two cameras):**

1. One S7, nothing else
2. One S3, nothing else
3. One S3 **and** one S7

**Not supported:** two S7s, two S3s, or any N>2. If a second body of the same model appears, ignore it (keep the first by USB serial) and do not try to share `192.168.129.10`.

**Bar glyph (colour, not a number disc):**

| Plugged | Glyph |
|---|---|
| none | outline camera, dim |
| S7 only | filled camera in **accent**; full opacity if TCP is up, ~55% if USB-only |
| S3 only | filled camera in **hue-shifted accent** (~150°) |
| S3 + S7 | two overlapping cameras: S3 colour behind, S7 accent in front |

`glyphCode` 3/7/10 is still the internal inventory code; it is not drawn.

Video stays UVC. Settings stay Yunxi protobuf on TCP 12345. One bar widget, one `yolocam watch`. We do not vendor [yolocam-linux](https://gitlab.com/yolocam-linux/yolocam-linux) (GPLv3). We reuse its published facts: S3 PID `46f8:0855`, subnet `192.168.126.10/24`, CID map, heartbeat, telnet recover.

## Is this easier than full multi-cam?

**Yes. This is the job we should do.** Full multi-cam was dominated by a problem this scope deletes.

Two S7s (or two S3s) are DHCP servers that both claim the **same** camera address on two USB Ethernet links:

```
S7 A  192.168.129.10:12345  on  enp…u2u4
S7 B  192.168.129.10:12345  on  enp…u1u3
```

Linux has one routing table. `connect(192.168.129.10:12345)` reaches one body; the other looks dead. The fix is `SO_BINDTODEVICE`, MAC-collision handling, per-serial preview dirs, a picker for N identical models, and a count badge. That is most of the old plan.

One S3 + one S7 do **not** collide:

```
S7  192.168.129.10:12345   CDC Ethernet   PID 0851
S3  192.168.126.10:12345   CDC NCM        PID 0855
```

Different `/24`s. Ordinary `connect()` works. Bind-to-device is a nice extra, not a gate.

What remains is still real work (second PID, two sessions when mixed, UVC pairing, S3 CID probe, a picker, the 3/7/10 disc) but it is “two known gadgets,” not “N identical routes.” Estimate: roughly half the risk of the old plan, and the scary networking chapter goes away.

```
Full multi-cam                         This programme
----------------                       ----------------
N S7s + N S3s                          at most one of each
duplicate 192.168.129.10               never (different /24s)
SO_BINDTODEVICE required               optional hardening
count badge N                          model code 3 / 7 / 10
picker of identical serials            picker of S3 vs S7
```

## Non-goals

- Two S7s or two S3s
- Firmware update, CLUT colour-wheel, YoloLiv remote assistance
- Dual preview (one viewfinder, selected camera only)
- Changing 160 ms RPC pace, click-to-start preview, or “do not auto-restart on GET timeout”
- Importing Python `yolocam-lib`

## Compatibility with today’s S7

A lone S7 must keep **control behaviour**: same protobuf, pace, heartbeat, recover, click-to-start preview, OBS-busy, Focus overlay, IPC `iso` / `shutter` / `focusMode` / `zoom`.

The **glyph is allowed to change**: today’s lone S7 has no disc; this plan puts **7** on it. Tooltip becomes `YoloCam S7 · connected` (or USB-only / unplugged as now). Panel title stays `YoloCam S7` when only the S7 is selected. No picker when N=1.

`status` JSON keeps today’s top-level keys for the selected/primary camera. A `cameras` array is additive.

## S3 vs S7 (unchanged facts)

| | S7 (probed here) | S3 (yolocam-linux) |
|---|---|---|
| USB VID:PID | `46f8:0851` | `46f8:0855` |
| USB net | `cdc_ether` | `cdc_ncm` |
| Camera IP | `192.168.129.10` | `192.168.126.10` |
| Host IP | `192.168.129.11` | `192.168.126.11` |
| Control | TCP 12345, same dual-envelope protobuf | same |
| Heartbeat | CID 215 ~25–30 s | same |
| Recover | telnet `root` / `yunxi.tv`, `live_camera_app` | same |
| Lens | MFT, ToF, CID 26/27 distortion/shading | Fixed; aperture index 0–31 |
| UVC | MJPEG 1280×720+ on this unit | Probe on real hardware |

Userspace does not care ECM vs NCM. After DHCP both are a `net/` iface. S3 support is a second PID + a second default `/24` + a CID allow-list, not a second protocol stack.

CID 26/27 are S7 Image-tab extras. On S3, GET will likely return `resp_code=100`. Hide those rows when the selected model does not support them.

## Discovery

Replace “first `0851`” with “at most one S7 and at most one S3.”

```
/sys/bus/usb/devices/*
    idVendor 46f8
    idProduct 0851  →  slot S7  (keep first serial if a second 0851 appears)
    idProduct 0855  →  slot S3  (keep first serial if a second 0855 appears)

For each slot:
    walk sysfs for net/          → iface, host IPv4, camera = sibling .10
    walk video4linux by USB parent of this device  → /dev/videoN
```

Pair UVC by walking `/sys/class/video4linux/videoN/device` up to the USB node. Do not take “first name containing yolocam” — that is how a mixed dock would attach the S7 preview to the S3 node.

Default camera IP if DHCP is late: S7 `192.168.129.10`, S3 `192.168.126.10`.

Inventory is 0, 1, or 2 cameras. Glyph code:

```
code = 0
if s3 { code += 3 }
if s7 { code += 7 }
# 0 none, 3 S3, 7 S7, 10 both
```

## Architecture

One `yolocam watch`. Zero, one, or two sessions. One `ctl.sock`. Events carry `id` (USB serial) and `model` (`s3`|`s7`).

```
Panel / Service
  selected = S7 or S3
  glyph disc = 3 | 7 | 10
        |
        v
yolocam watch
  Session S7  →  192.168.129.10:12345   preview …/yolocam/<s7-serial>/
  Session S3  →  192.168.126.10:12345   preview …/yolocam/<s3-serial>/
        |
        +-- each: 160 ms pace, own heartbeat, own Fill, own recover
```

When only one body is present, watch looks like today (one TCP client, one preview dir may stay `$XDG_RUNTIME_DIR/yolocam` **or** move under serial — prefer serial always so mixed does not special-case).

`select` chooses which session the panel’s `values` / preview / AF overlay mirror. IPC without an id talks to the selected camera (default: the only one, or last selected, or S7 if both just appeared — this dock’s native body).

Preview: still click-to-start, one STREAMON at a time. Switching selection stops the other preview. OBS holding the S7 node only marks the S7 preview busy; S3 controls and preview stay available.

Recover: `yolocam recover` without id recovers the selected camera; with `--model s3|s7` the named one. Telnet to that camera’s `.10` on its own `/24` — no bind-to-device required for correctness.

Optional later: `SO_BINDTODEVICE` anyway. Not a Phase-1 gate.

## Glyph and panel

**Bar**

| State | Glyph | Colour |
|---|---|---|
| unplugged | outline `󰵝` | dim |
| S7 | filled `󰄀` | accent; 55% opacity if USB-only |
| S3 | filled `󰄀` | accent hue-shifted ~150° |
| both | two overlapping `󰄀` | S3 colour behind, S7 accent in front |

Tooltip examples: `YoloCam S7 · connected`, `YoloCam S3 · USB only`, `YoloCam S3+S7 · 2 connected`.

Right-click AF toggle applies to the **selected** camera.

**Panel**

N=1: no picker. Title `YoloCam S7` or `YoloCam S3`.

N=2: compact S3 | S7 switch under the hero (two models, not a serial list). Changing it sends `select`. Keep the current tab id if that tab exists on the other model; if a row is S7-only (distortion), hide it rather than fail.

## Phased delivery

Each phase must leave a docked lone S7 fully controllable. The new **7** disc may land in the same phase as S3 detection so we do not ship a badge that then changes meaning.

### Phase 0 — S3 protocol spike (hardware gate)

Needs a physical S3.

- USB `46f8:0855`, `cdc_ncm`, DHCP `192.168.126.11` / `.10`
- Existing `wire.rs` / `pb.rs` GET CID 1
- MJPEG size for preview
- Which panel CIDs return 200 vs 100

Write `docs/s3-protocol.md`. If framing differs, stop. If no S3 is on the desk, Phase 1 can still add S7-only discovery + the **7** disc; S3 slots stay empty until Phase 0.

### Phase 1 — Dual discovery, one selected session

- `discover_s7()` / `discover_s3()`; ignore extra same-PID devices
- Pair each UVC node to its USB parent
- Watch: 0–2 sessions, events tagged `id` + `model`
- Service: `cameras[]`, `selectedModel`; flat properties = selected
- Glyph: 3 / 7 / 10 / none
- Panel picker only when both slots filled
- Preview dirs per serial
- Lone S7: control path identical; disc shows **7**

### Phase 2 — Mixed dock polish

- Persist last selected model
- Unplug selected → fall back to the remaining body, disc updates 10→3 or 10→7
- Second same-model plug: log and ignore
- Status tab shows model, iface, `/24`, firmware of the selected body
- Manifest name “YoloCam”; aliases keep `s7` and add `s3`

### Phase 3 — Optional hardening

- `SO_BINDTODEVICE`
- NM never-default on both gadget NICs
- CLI `--model s3|s7`

## Test plan

Automated:

- Inventory fixtures: empty, S7, S3, both, two S7s (second ignored), two S3s (second ignored)
- Glyph code: 0 / 3 / 7 / 10
- Sibling `.10` on 126 and 129
- Pace still 160 ms **per session** (two heartbeats must not share one clock that starves the S3)

On this dock, every merge:

- Lone S7: preview, OBS busy, AF overlay, set ISO, unplug/replug, recover, accent glyph

When an S3 exists:

- Lone S3: connect, GET-all, preview, shifted-accent glyph
- Mix: stacked two-colour glyph, picker, S7 ISO does not change S3, OBS on S7 does not block S3 preview

## Risks

| Risk | Why it is smaller now | Mitigation |
|---|---|---|
| Duplicate `.10` on two S7s | Out of scope | Ignore second `0851` |
| S3 wire drift | Still real | Phase 0 spike |
| S3 UVC not 1280×720 | Still real | Negotiate MJPEG size |
| Two “YoloLiv YoloCam” video nodes | Still real on a mix | Pair by USB parent |
| `10` disc unreadable at 16px | New | Wider pill, tabular digits |
| NM default-route with two gadget NICs | Smaller than N identical NICs | never-default if it appears |
| Lone S7 users notice the new **7** | Intentional | Document as the model mark |

## Acceptance

- [ ] One S7: full current control behaviour; disc shows 7
- [ ] One S3: same panel; disc shows 3
- [ ] S3+S7: disc shows 10; picker; both sockets live; one preview at a time
- [ ] A second S7 or second S3 is ignored
- [ ] Unplug of the selected body falls back; disc updates
- [ ] OBS holding one UVC node only disables that body’s preview
