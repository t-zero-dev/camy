# Contributing

The QML panel hot-reloads on save. The helper does not.

```sh
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_DIR=/tmp/yolocam-target
cd helper
cargo test
cargo clippy --all-targets -- -D clippy::all
cargo build --release
cp /tmp/yolocam-target/release/yolocam ../bin/yolocam.new
mv -f ../bin/yolocam.new ../bin/yolocam
omarchy plugin validate ..
omarchy restart shell
```

Do not `cp` onto a running `bin/yolocam` (`ETXTBSY`). Use `yolocam.new` then `mv -f`.

Hard rules for the helper: one TCP 12345 client, 160 ms RPC pace, no auto-kill of `live_camera_app`, no `yolocam status` / `get` / `set` against a live watch. Use `yolocam ctl`.

`omarchy plugin validate` refuses any symlink in the plugin folder.
