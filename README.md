# MatrixShot

Wayland-native screenshot and screen-recording suite with a Matrix-themed Quickshell overlay.

**Print** → region select → **Screenshot | Screen Record** chooser (fast shot path, no extra confirm).
Ambxst keeps `SUPER+S` / `SUPER+SHIFT+S` / `SUPER+SHIFT+R` — do not steal those binds.

**Status:** v0.2.1 — chooser + record config UI, stable audio IDs, codec `-k`, focused-monitor vs fullscreen, Nix packaging ready.

## Features

- Region + fullscreen screenshots via grim/slurp
- Post-region chooser: Screenshot | Screen Record
- Compact recording config (fps, audio none/desktop/mic/both, output dir)
- gpu-screen-recorder orchestration with REC indicator overlay
- Quickshell annotation editor (pen / highlight / rect / arrow / text)
- Upload last capture (catbox / 0x0 / litterbox / imgur)

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
