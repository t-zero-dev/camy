# Contributing

The QML panel hot-reloads on save. The helper does not.

```sh
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/camy/target"
cd helper
cargo test
cargo clippy --all-targets -- -D clippy::all
cd ..
bash scripts/build-helper.sh
omarchy plugin validate ..
omarchy restart shell
```

`scripts/build-helper.sh` owns the release install (`*.new` then `mv -f`) so a running helper is not overwritten (`ETXTBSY`). Do not copy out of `/tmp`.

Hard rules for the helper: one TCP 12345 client, 160 ms RPC pace, no auto-kill of `live_camera_app`, no `camy status` / `get` / `set` against a live watch. Use `camy ctl`.

`omarchy plugin validate` refuses any symlink in the plugin folder.
