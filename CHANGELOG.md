# Changelog

## Unreleased
- Honor disabled clipboard copying without requiring wl-copy; warn on copy failure while preserving the capture.
- Add isolated screenshot CLI regression tests and Rust CI (format, Clippy, tests, release build, CLI smoke).
- Resolve existing formatting and Clippy findings.

## 0.2.3 — 2026-09-29
- Fix upload hangs: parallel race across providers with `--connect-timeout 3` / `--max-time 45` / `-4`
- Default provider `uguu` (catbox/litterbox often TCP-blackhole; 0x0 currently 503 uploads-disabled)
- Race catbox / 0x0 / litterbox in parallel; `tmpfiles` / `imgur` remain opt-in; clearer multi-host errors
- `require_bin` falls back to `~/.local/bin` and NixOS `/run/current-system/sw/bin` (Hyprland PATH)

## 0.2.2 — 2026-09-29
- Fix Quickshell overlay crash: rename theme id `T` → `theme` (QML/Quickshell forbids IDs starting with uppercase), restoring Screenshot | Screen Record chooser

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