# Changelog

## 0.2.1 — 2026-09-29
- Wire `recording.codec` / CLI `--codec`/`-k` through to gpu-screen-recorder (skip when `auto`)
- `record monitor` uses `-w focused`; `record fullscreen` keeps `-w screen`


## 0.2.0 — 2026-09-29
- Post-region chooser: `matrixshot choose` (Print) → Screenshot | Screen Record
- Compact recording config UI (fps, audio none/desktop/mic/both, output dir, start)
- Record uses stable audio IDs (`default_output` / `default_input` / both); `record list-audio`
- Geometry bridge: slurp/grim `X,Y WxH` ↔ gsr `WxH+X+Y`
- Honor `selection.border` and `preview.enabled`
- Quickshell helpers: `matrixshot-ui`, packaging under `packaging/nix/package.nix`
- NixOS package entrypoints filled (rebuild still required for system profile)

## 0.1.0
- Initial CLI + Quickshell preview/editor/REC overlay