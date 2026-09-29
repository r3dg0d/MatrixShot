import QtQuick

// Shared Matrix × DotSquared tokens for MatrixShot Quickshell surfaces.
// Keep in sync with:
//   ~/dotfiles/config/theme-seeds/zionsec-matrix-tokens.json
//   /home/neo/Projects/_megaprompt-inventory/theme/zionsec-matrix-tokens.json
//   packages/desktop-tools/qml/shared/Theme.qml (matrix* fallbacks)
QtObject {
    // Near-black surfaces
    readonly property color bg: "#050805"
    readonly property color bgOled: "#000000"
    readonly property color surface: "#0a100c"
    readonly property color surfaceRaised: "#0f1812"
    readonly property color surfaceHigh: "#152019"
    readonly property color surfaceBright: "#1c2a22"
    readonly property color panel: "#f0080c08"
    readonly property color overlay: "#cc050805"

    // Phosphor / Matrix accents (Ghostty CRT parity)
    readonly property color phosphor: "#00ff66"
    readonly property color phosphorDim: "#00cc55"
    readonly property color phosphorSoft: "#7dff9a"
    readonly property color phosphorText: "#b8ffb8"
    readonly property color primary: "#00c853"
    readonly property color primaryDim: "#1b5e20"
    readonly property color border: "#1a3d28"
    readonly property color borderMuted: "#1f5f1f"
    readonly property color borderStrong: "#1a4a1a"
    readonly property color borderHover: "#335533"

    // Status
    readonly property color success: "#69f0ae"
    readonly property color warn: "#ffb300"
    readonly property color critical: "#ff5252"
    readonly property color criticalSoft: "#ff8888"
    readonly property color criticalBg: "#2a1515"
    readonly property color muteBg: "#141414"
    readonly property color cyan: "#18ffff"
    readonly property color fg: "#d8ffe8"
    readonly property color fgDim: "#8fb89a"

    // Typography
    readonly property string mono: "JetBrainsMono Nerd Font Mono, JetBrains Mono, monospace"
    readonly property int fs: 12
    readonly property int fsSmall: 10
    readonly property int fsTiny: 11
}
