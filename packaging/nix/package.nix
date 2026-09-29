{ lib
, rustPlatform
, runCommand
, writeShellApplication
, symlinkJoin
, makeWrapper
, quickshell
, grim
, slurp
, wl-clipboard
, gpu-screen-recorder
, curl
, libnotify
, imagemagick
, matrixshotSrc ? null
}:
let
  version = "0.2.1";

  # Prefer explicit src (Projects path / flake); fall back to files next to this
  # expression when vendored under packages/matrixshot.
  srcRoot =
    if matrixshotSrc != null then matrixshotSrc
    else ./../..; # Projects/MatrixShot when called from packaging/nix/

  cli = rustPlatform.buildRustPackage {
    pname = "matrixshot";
    inherit version;
    src = lib.cleanSourceWith {
      src = srcRoot;
      filter = path: type:
        let base = baseNameOf path; in
        !(lib.hasInfix "/target/" path)
        && !(lib.hasInfix "/.git/" path)
        && !(lib.hasSuffix ".bak" base)
        && !(lib.hasInfix ".bak-" base)
        && base != "target"
        && base != ".git";
    };
    cargoLock.lockFile = srcRoot + "/Cargo.lock";
    doCheck = false;
    meta = {
      description = "Wayland screenshot and screen-recording suite";
      license = lib.licenses.mit;
      mainProgram = "matrixshot";
      platforms = lib.platforms.linux;
    };
  };

  qml = runCommand "matrixshot-qml-${version}" { } ''
    mkdir -p $out/share/matrixshot/quickshell
    cp -r ${srcRoot}/quickshell/. $out/share/matrixshot/quickshell/
    find $out/share/matrixshot/quickshell -name '*.bak*' -delete
  '';
  qmlDir = "${qml}/share/matrixshot/quickshell";

  # Shared overlay launcher — prefers MATRIXSHOT_QS_DIR, then packaged share.
  mkHelper = name: body: writeShellApplication {
    inherit name;
    runtimeInputs = [ quickshell ];
    text = ''
      export MATRIXSHOT_QS_DIR="''${MATRIXSHOT_QS_DIR:-${qmlDir}}"
      ${body}
    '';
  };

  matrixshot-ui = mkHelper "matrixshot-ui" ''
    QS=$(command -v qs || command -v quickshell || true)
    SHELL_DIR="''${MATRIXSHOT_QS_DIR}"
    case "''${1:-ensure}" in
      ensure|start)
        [[ -n "$QS" && -f "$SHELL_DIR/shell.qml" ]] || exit 0
        "$QS" -n -d -p "$SHELL_DIR" >/dev/null 2>&1 \
          || "$QS" -d -p "$SHELL_DIR" >/dev/null 2>&1 \
          || true
        ;;
      *) echo "usage: matrixshot-ui ensure"; exit 1 ;;
    esac
  '';

  matrixshot-preview = mkHelper "matrixshot-preview" ''
    path="''${1:-}"
    [[ -n "$path" && -f "$path" ]] || exit 0
    state="''${XDG_STATE_HOME:-$HOME/.local/state}/matrixshot"
    mkdir -p "$state"
    name=$(basename "$path")
    dims=""
    if command -v identify >/dev/null 2>&1; then
      dims=$(identify -format '%wx%h' "$path" 2>/dev/null || true)
    fi
    timeout=10
    cfg="''${XDG_CONFIG_HOME:-$HOME/.config}/matrixshot/config.toml"
    if [[ -f "$cfg" ]]; then
      timeout=$(grep -oE 'timeout[[:space:]]*=[[:space:]]*[0-9]+' "$cfg" | head -1 | grep -oE '[0-9]+' || echo 10)
    fi
    printf '{"path":"%s","name":"%s","dims":"%s","timeout":%s}\n' \
      "$path" "$name" "$dims" "$timeout" >"$state/preview.json"
    matrixshot-ui ensure || true
  '';

  matrixshot-edit = mkHelper "matrixshot-edit" ''
    path="''${1:-}"
    [[ -n "$path" && -f "$path" ]] || { echo "usage: matrixshot-edit IMAGE.png" >&2; exit 1; }
    state="''${XDG_STATE_HOME:-$HOME/.local/state}/matrixshot"
    mkdir -p "$state"
    printf '{"path":"%s"}\n' "$path" >"$state/edit.json"
    matrixshot-ui ensure || true
  '';

  matrixshot-rec-ui = mkHelper "matrixshot-rec-ui" ''
    state="''${XDG_STATE_HOME:-$HOME/.local/state}/matrixshot"
    mkdir -p "$state"
    case "''${1:-}" in
      start)
        printf '{"active":true,"startedAt":%s,"path":"%s"}\n' "$(date +%s)" "''${2:-}" >"$state/recording.json"
        ;;
      stop)
        printf '{"active":false}\n' >"$state/recording.json"
        ;;
      *) echo "usage: matrixshot-rec-ui start|stop [path]"; exit 1 ;;
    esac
    matrixshot-ui ensure || true
  '';

  # Wrap CLI so sibling helpers resolve next to it via PATH order.
  matrixshotWrapped = runCommand "matrixshot-wrapped-${version}" {
    nativeBuildInputs = [ makeWrapper ];
  } ''
    mkdir -p $out/bin
    makeWrapper ${cli}/bin/matrixshot $out/bin/matrixshot \
      --prefix PATH : ${lib.makeBinPath [
        grim slurp wl-clipboard gpu-screen-recorder curl libnotify
        matrixshot-ui matrixshot-preview matrixshot-edit matrixshot-rec-ui
      ]}
  '';
in
symlinkJoin {
  name = "matrixshot-${version}";
  paths = [
    matrixshotWrapped
    qml
    matrixshot-ui
    matrixshot-preview
    matrixshot-edit
    matrixshot-rec-ui
  ];
  passthru = {
    inherit cli qml qmlDir;
  };
  meta = {
    description = "MatrixShot CLI + Quickshell overlay helpers";
    mainProgram = "matrixshot";
  };
}
