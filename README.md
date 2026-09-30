# MatrixShot

Wayland-native screenshot and screen-recording suite with a Matrix-themed Quickshell overlay.

**Print** → region select → **Screenshot | Screen Record** chooser (fast shot path, no extra confirm).
Ambxst keeps `SUPER+S` / `SUPER+SHIFT+S` / `SUPER+SHIFT+R` — do not steal those binds.

**Status:** v0.2.3 — fast upload race (uguu default + catbox/0x0/litterbox/tmpfiles failover), chooser + record UI, Nix packaging ready.

## Features

- Region + fullscreen screenshots via grim/slurp
- Post-region chooser: Screenshot | Screen Record
- Compact recording config (fps, audio none/desktop/mic/both, output dir)
- gpu-screen-recorder orchestration with REC indicator overlay
- Quickshell annotation editor (pen / highlight / rect / arrow / text)
- Upload last capture (uguu default; races catbox / 0x0 / litterbox; copies URL via wl-copy)

## CLI

```
matrixshot choose                         # Print default: slurp → chooser
matrixshot region [--geometry GEO]        # immediate region shot
matrixshot fullscreen
matrixshot record region --geometry GEO --fps 60 --audio desktop|mic|both|none [-k CODEC]
matrixshot record monitor   # focused output (-w focused)
matrixshot record fullscreen  # all screens (-w screen)
matrixshot record list-audio
matrixshot record stop|status|toggle
matrixshot upload-last | folder | open-last | config
```

Geometry: slurp/grim use `X,Y WxH`; gsr uses `WxH+X+Y` — MatrixShot converts both ways.

## Config

`~/.config/matrixshot/config.toml` — created on first run.

Clipboard copying is optional: set `screenshot.copy_to_clipboard = false` to
capture without `wl-copy`. When copying is enabled but unavailable or fails, the
capture is still saved and registered as the last capture; a warning explains the
clipboard failure.

## Recording lifecycle

Recording requires Linux 5.3 or newer with pidfd support. MatrixShot saves the
recorder's boot identity and process start time alongside its PID, then uses a
pidfd to signal that verified process. Stale or unverified state is cleared
without signaling the saved PID. After upgrading from a PID-only version, stop
any existing recorder manually before starting a new session.

`record stop` waits up to five seconds for the recorder to exit. If it is still
finishing, the command fails and retains state; retry `record stop` later. Only
after exit is a nonempty output file registered as the last capture. Immediate
startup failures clear recording state and preserve the previous last capture.

## Development checks

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release
```

CI runs these checks and CLI smoke tests. Integration tests use fake capture,
clipboard and recorder commands with isolated config and state directories.
They cover process identity, startup failure and delayed shutdown without a
compositor. Live capture and recording still require Wayland and
the corresponding external tools.

## Nix packaging

```
Projects/MatrixShot/packaging/nix/package.nix
/etc/nixos/packages/matrixshot/{default,package}.nix
modules/matrixshot-path.nix  # callPackage when Projects tree present
```

`nixos-rebuild switch` is still required to put the store package on the system profile.
Until then, `~/.local/bin/matrixshot*` is the live path.

## License

MIT — see LICENSE.
