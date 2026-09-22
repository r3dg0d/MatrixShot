import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland
import Quickshell.Io

Scope {
    id: root

    property var preview: ({})
    property var recording: ({ active: false, startedAt: 0, path: "" })

    FileView {
        id: previewFile
        path: Quickshell.env("HOME") + "/.local/state/matrixshot/preview.json"
        watchChanges: true
        onFileChanged: previewFile.reload()
        onLoaded: {
            try {
                root.preview = JSON.parse(previewFile.text())
            } catch (e) {
                root.preview = ({})
            }
        }
    }

    FileView {
        id: recordFile
        path: Quickshell.env("HOME") + "/.local/state/matrixshot/recording.json"
        watchChanges: true
        onFileChanged: recordFile.reload()
        onLoaded: {
            try {
                root.recording = JSON.parse(recordFile.text())
            } catch (e) {
                root.recording = ({ active: false })
            }
        }
    }

    // Screenshot preview card — top-right, non-focus-stealing overlay
    PanelWindow {
        id: previewWin
        visible: !!(root.preview && root.preview.path)
        screen: Quickshell.screens.length > 0 ? Quickshell.screens[0] : null
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
        exclusiveZone: 0
        color: "transparent"
        anchors { top: true; right: true }
        margins { top: 12; right: 12 }
        implicitWidth: 320
        implicitHeight: card.implicitHeight

        Rectangle {
            id: card
            anchors.fill: parent
            radius: 12
            color: "#e6101010"
            border.color: "#00ff00"
            border.width: 1
            implicitHeight: col.implicitHeight + 24

            ColumnLayout {
                id: col
                anchors.fill: parent
                anchors.margins: 12
                spacing: 8

                RowLayout {
                    Layout.fillWidth: true
                    Text {
                        text: "MatrixShot"
                        color: "#00ff00"
                        font.pixelSize: 13
                        font.bold: true
                        Layout.fillWidth: true
                    }
                    Button {
                        text: "✕"
                        flat: true
                        onClicked: {
                            root.preview = ({})
                            Quickshell.execDetached(["rm", "-f", Quickshell.env("HOME") + "/.local/state/matrixshot/preview.json"])
                        }
                    }
                }

                Image {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 140
                    fillMode: Image.PreserveAspectFit
                    source: root.preview.path ? ("file://" + root.preview.path) : ""
                    asynchronous: true
                }

                Text {
                    text: (root.preview.name || "") + (root.preview.dims ? (" · " + root.preview.dims) : "")
                    color: "#cccccc"
                    font.pixelSize: 12
                    elide: Text.ElideMiddle
                    Layout.fillWidth: true
                }

                RowLayout {
                    spacing: 6
                    Button { text: "Open"; onClicked: Quickshell.execDetached(["imv", root.preview.path]) }
                    Button { text: "Folder"; onClicked: Quickshell.execDetached(["matrixshot", "folder"]) }
                    Button {
                        text: "Edit"
                        onClicked: Quickshell.execDetached(["sh", "-c", "command -v gimp >/dev/null && gimp \"$1\" || command -v krita >/dev/null && krita \"$1\" || imv \"$1\"", "sh", root.preview.path])
                    }
                    Button {
                        text: "Upload"
                        onClicked: Quickshell.execDetached(["matrixshot", "upload-last"])
                    }
                }
            }

            Timer {
                id: dismiss
                interval: (root.preview.timeout || 10) * 1000
                running: previewWin.visible
                onTriggered: {
                    root.preview = ({})
                    Quickshell.execDetached(["rm", "-f", Quickshell.env("HOME") + "/.local/state/matrixshot/preview.json"])
                }
            }
            // pause timer while hovering
            MouseArea {
                anchors.fill: parent
                hoverEnabled: true
                acceptedButtons: Qt.NoButton
                onEntered: dismiss.stop()
                onExited: { if (previewWin.visible) dismiss.restart() }
            }
        }
    }

    // Recording indicator
    PanelWindow {
        id: recWin
        visible: !!(root.recording && root.recording.active)
        screen: Quickshell.screens.length > 0 ? Quickshell.screens[0] : null
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
        exclusiveZone: 0
        color: "transparent"
        anchors { top: true; left: true }
        margins { top: 12; left: 12 }
        implicitWidth: recCard.implicitWidth
        implicitHeight: recCard.implicitHeight

        Rectangle {
            id: recCard
            radius: 10
            color: "#e6101010"
            border.color: "#ff3333"
            border.width: 1
            implicitWidth: recRow.implicitWidth + 24
            implicitHeight: recRow.implicitHeight + 16

            RowLayout {
                id: recRow
                anchors.centerIn: parent
                spacing: 10
                Text { text: "● REC"; color: "#ff3333"; font.bold: true; font.pixelSize: 14 }
                Text {
                    id: elapsed
                    color: "#eeeeee"
                    font.pixelSize: 14
                    text: "00:00:00"
                }
                Button {
                    text: "Stop"
                    onClicked: Quickshell.execDetached(["matrixshot", "record", "stop"])
                }
            }

            Timer {
                interval: 500
                running: recWin.visible
                repeat: true
                onTriggered: {
                    const start = root.recording.startedAt || 0
                    if (!start) return
                    const sec = Math.max(0, Math.floor(Date.now() / 1000 - start))
                    const h = String(Math.floor(sec / 3600)).padStart(2, "0")
                    const m = String(Math.floor((sec % 3600) / 60)).padStart(2, "0")
                    const s = String(sec % 60).padStart(2, "0")
                    elapsed.text = h + ":" + m + ":" + s
                }
            }
        }
    }
}
