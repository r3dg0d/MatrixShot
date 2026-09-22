# MatrixShot

Wayland-native screenshot and screen-recording suite for Linux.

**Status:** v0.1.0 MVP — CLI screenshots and gpu-screen-recorder orchestration work. Quickshell preview/REC UI and uploads are in progress.

## Features

- Region + fullscreen screenshots via `grim` + `slurp`
- Matrix-green (`#00ff00`) selection border
- Clipboard copy via `wl-copy`
- XDG save paths (`~/Pictures/Screenshots`, `~/Videos/MatrixShot`)
- Recording via `gpu-screen-recorder` (start/stop/toggle/status)
- Opens captures with `imv` (configurable)
- Manual-only uploads (disabled by default)

## Privacy

- No automatic uploads
- No telemetry
- No hardcoded secrets

## CLI

```bash
matrixshot              # region (default)
matrixshot region
matrixshot fullscreen
matrixshot open-last
matrixshot folder
matrixshot record toggle
matrixshot record status
matrixshot --help
```

## Nix

```bash
nix run .#
nix develop
nix flake check
```

## Dependencies

`grim`, `slurp`, `wl-clipboard`, `gpu-screen-recorder`, `imv` (viewer).

## License

MIT
