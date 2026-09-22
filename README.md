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
- Edit opens a Quickshell annotation widget (not Krita/GIMP)
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


## Upload

Preview **Upload** posts the last capture and copies the URL to the clipboard (`wl-copy`).

```toml
[upload]
enabled = true
provider = "catbox"   # catbox | 0x0 | litterbox | imgur
copy_url = true
imgur_client_id = ""  # required only for imgur
```

MatrixShot tries the configured provider first, then falls back to `catbox` → `0x0` → `litterbox` if the host errors.

## License

MIT
