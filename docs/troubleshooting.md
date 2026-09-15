# Troubleshooting

## USB present, control refused

`yolocam status` (or the panel) shows the camera on USB but TCP `192.168.129.10:12345` or `192.168.126.10:12345` is connection-refused. `live_camera_app` on the camera has stalled.

```bash
~/.config/omarchy/plugins/yolocam/bin/yolocam recover
```

That telnets to the camera (`root` / `yunxi.tv` on the gadget NIC), restarts `/app/bin/live_camera_app`, and waits until 12345 accepts. If recover cannot reach the gadget network, unplug data **and** power, then plug both back.

Do not loop GET-all or auto-recover on GET timeout. That is what wedges the panel.

## Panel hung on Refresh

Refresh is tab-only and urgent. If it still never finishes, the control socket is likely dead — recover, then reopen the panel. Watch will not restart the camera by itself.

## Preview says busy

UVC is exclusive. OBS, Chromium/Meet, or Zoom holding `/dev/videoN` makes STREAMON return busy. Controls still work. Close the other capture or pick the other camera on a mixed dock (OBS on the S7 does not block S3 preview).

## Preview click does nothing

Preview is click-to-start and only while the panel is open. S3 STREAMON can take several seconds on the SoC; wait before clicking again. Confirm the widget setting “Live preview when the panel is open” is on.

## Second client / `yolocam status` while the bar is up

Watch holds 12345. A second `get`, `set`, `probe`, or firmware `status` opens another TCP client and can kill `live_camera_app`. Use `yolocam ctl` or the panel. `status` is safe for USB inventory only when it skips the control probe because watch owns the socket.

## Mixed S3 + S7

Different `/24`s, so ordinary `connect()` works. Glyph: two overlapping cameras, selected model in front. If ISO on one body seems to move the other, check the S3/S7 picker — commands without `model` hit the selected session.

A second S7 or second S3 is ignored (same `.10` address). Unplug the extra body.

## Gadget NIC became the default route

Unusual if Wi-Fi or Ethernet already has default. Mark the USB NIC never-default in NetworkManager if it happens. The camera network is only for control.

## HDMI-only S7 looks missing

The kernel must bind `46f8:0851` (or `0855`). HDMI out without the USB 3 **data** cable is not a control path.

## S7 rebooted after a SET

Only named parameters are SET. CID 55, 58, and 29 are not in that table; writing them has rebooted an S7. If a fork adds a CID, probe with GET first and never SET an undocumented integer on the S7.

## Build / enable

```bash
export CARGO_TARGET_DIR=/tmp/yolocam-target
cd ~/.config/omarchy/plugins/yolocam/helper
cargo test
cargo build --release
cp -f /tmp/yolocam-target/release/yolocam ../bin/yolocam
omarchy plugin validate ~/.config/omarchy/plugins/yolocam
```

`omarchy plugin validate` fails if `target/` (or a symlink to it) sits inside the plugin directory. After replacing the binary, restart watch (close the panel so the service can relaunch, or `omarchy restart shell`).
