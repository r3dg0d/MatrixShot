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
    property var upload: ({ status: "", url: "", error: "" })
    property string editPath: ""
    readonly property string bin: Quickshell.env("HOME") + "/.local/bin/matrixshot"
    readonly property string iconDir: Qt.resolvedUrl("./icons/")

    FileView {
        id: previewFile
        path: Quickshell.env("HOME") + "/.local/state/matrixshot/preview.json"
        watchChanges: true
        onFileChanged: previewFile.reload()
        onLoaded: {
            try { root.preview = JSON.parse(previewFile.text()) }
            catch (e) { root.preview = ({}) }
        }
    }

    FileView {
        id: recordFile
        path: Quickshell.env("HOME") + "/.local/state/matrixshot/recording.json"
        watchChanges: true
        onFileChanged: recordFile.reload()
        onLoaded: {
            try { root.recording = JSON.parse(recordFile.text()) }
            catch (e) { root.recording = ({ active: false }) }
        }
    }

    FileView {
        id: uploadFile
        path: Quickshell.env("HOME") + "/.local/state/matrixshot/upload.json"
        watchChanges: true
        onFileChanged: uploadFile.reload()
        onLoaded: {
            try { root.upload = JSON.parse(uploadFile.text()) }
            catch (e) { root.upload = ({ status: "", url: "", error: "" }) }
        }
    }

    FileView {
        id: editRequest
        path: Quickshell.env("HOME") + "/.local/state/matrixshot/edit.json"
        watchChanges: true
        onFileChanged: editRequest.reload()
        onLoaded: {
            try {
                const j = JSON.parse(editRequest.text())
                if (j && j.path) root.editPath = j.path
            } catch (e) {}
        }
    }

    function openEditor(path) {
        dismiss.stop()
        root.preview = ({})
        root.editPath = path
        Quickshell.execDetached(["rm", "-f", Quickshell.env("HOME") + "/.local/state/matrixshot/preview.json"])
    }

    component MatrixIconButton: Rectangle {
        id: btn
        property string label: ""
        property string iconName: ""
        property bool busy: false
        property bool primary: false
        signal clicked()

        Layout.fillWidth: true
        Layout.preferredHeight: 36
        radius: 8
        color: ma.containsMouse ? (primary ? "#1a3d1a" : "#1a1a1a") : (primary ? "#122612" : "#141414")
        border.color: primary ? "#00ff66" : (ma.containsMouse ? "#00cc55" : "#1f5f1f")
        border.width: 1
        opacity: ma.pressed ? 0.75 : 1

        RowLayout {
            anchors.centerIn: parent
            spacing: 6
            Image {
                visible: btn.iconName.length > 0
                source: root.iconDir + btn.iconName + ".svg"
                sourceSize.width: 16
                sourceSize.height: 16
                width: 16
                height: 16
                fillMode: Image.PreserveAspectFit
                opacity: btn.busy ? 0.45 : 1
            }
            Text {
                text: btn.busy ? "…" : btn.label
                color: "#b8ffb8"
                font.pixelSize: 12
                font.bold: true
                font.family: "monospace"
            }
        }

        MouseArea {
            id: ma
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: if (!btn.busy) btn.clicked()
        }
    }

    // Screenshot preview card — top-right
    PanelWindow {
        id: previewWin
        visible: !!(root.preview && root.preview.path) && root.editPath.length === 0
        screen: Quickshell.screens.length > 0 ? Quickshell.screens[0] : null
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
        exclusiveZone: 0
        color: "transparent"
        anchors { top: true; right: true }
        margins { top: 12; right: 12 }
        implicitWidth: 340
        implicitHeight: card.implicitHeight

        Rectangle {
            id: card
            anchors.fill: parent
            radius: 12
            color: "#f0080c08"
            border.color: "#00ff66"
            border.width: 1
            implicitHeight: col.implicitHeight + 24

            // subtle inner glow line
            Rectangle {
                anchors.fill: parent
                anchors.margins: 1
                radius: 11
                color: "transparent"
                border.color: "#2200ff66"
                border.width: 1
            }

            ColumnLayout {
                id: col
                anchors.fill: parent
                anchors.margins: 12
                spacing: 10

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    Text {
                        text: "MATRIXSHOT"
                        color: "#00ff66"
                        font.pixelSize: 12
                        font.bold: true
                        font.letterSpacing: 1.5
                        font.family: "monospace"
                        Layout.fillWidth: true
                    }
                    Rectangle {
                        width: 28
                        height: 28
                        radius: 6
                        color: closeMa.containsMouse ? "#2a1515" : "#141414"
                        border.color: closeMa.containsMouse ? "#ff5555" : "#335533"
                        border.width: 1
                        Image {
                            anchors.centerIn: parent
                            source: root.iconDir + "close.svg"
                            sourceSize.width: 14
                            sourceSize.height: 14
                            width: 14
                            height: 14
                        }
                        MouseArea {
                            id: closeMa
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                root.preview = ({})
                                root.upload = ({ status: "", url: "", error: "" })
                                Quickshell.execDetached(["rm", "-f", Quickshell.env("HOME") + "/.local/state/matrixshot/preview.json"])
                                Quickshell.execDetached(["rm", "-f", Quickshell.env("HOME") + "/.local/state/matrixshot/upload.json"])
                            }
                        }
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 148
                    radius: 8
                    color: "#0a0a0a"
                    border.color: "#1a4a1a"
                    border.width: 1
                    clip: true
                    Image {
                        anchors.fill: parent
                        anchors.margins: 4
                        fillMode: Image.PreserveAspectFit
                        source: root.preview.path ? ("file://" + root.preview.path) : ""
                        asynchronous: true
                    }
                }

                Text {
                    text: (root.preview.name || "") + (root.preview.dims ? (" · " + root.preview.dims) : "")
                    color: "#7dff9a"
                    font.pixelSize: 11
                    font.family: "monospace"
                    elide: Text.ElideMiddle
                    Layout.fillWidth: true
                }

                GridLayout {
                    Layout.fillWidth: true
                    columns: 2
                    rowSpacing: 6
                    columnSpacing: 6

                    MatrixIconButton {
                        label: "Open"
                        iconName: "open"
                        onClicked: Quickshell.execDetached(["imv", root.preview.path])
                    }
                    MatrixIconButton {
                        label: "Folder"
                        iconName: "folder"
                        onClicked: Quickshell.execDetached([root.bin, "folder"])
                    }
                    MatrixIconButton {
                        label: "Edit"
                        iconName: "edit"
                        onClicked: root.openEditor(root.preview.path)
                    }
                    MatrixIconButton {
                        label: "Upload"
                        iconName: "upload"
                        primary: true
                        busy: root.upload.status === "uploading"
                        onClicked: {
                            dismiss.stop()
                            root.upload = ({ status: "uploading", url: "", error: "" })
                            Quickshell.execDetached([root.bin, "upload-last"])
                        }
                    }
                }

                Rectangle {
                    visible: root.upload.status === "ok" || root.upload.status === "error" || root.upload.status === "uploading"
                    Layout.fillWidth: true
                    radius: 8
                    color: root.upload.status === "error" ? "#1a0808" : "#081408"
                    border.color: root.upload.status === "error" ? "#ff5555" : "#00ff66"
                    border.width: 1
                    implicitHeight: statusCol.implicitHeight + 16

                    ColumnLayout {
                        id: statusCol
                        anchors.fill: parent
                        anchors.margins: 8
                        spacing: 4
                        RowLayout {
                            spacing: 6
                            Image {
                                visible: root.upload.status === "ok"
                                source: root.iconDir + "check.svg"
                                sourceSize.width: 14
                                sourceSize.height: 14
                                width: 14
                                height: 14
                            }
                            Text {
                                text: root.upload.status === "uploading"
                                      ? "Uploading…"
                                      : (root.upload.status === "ok" ? "URL copied to clipboard" : "Upload failed")
                                color: root.upload.status === "error" ? "#ff8888" : "#00ff66"
                                font.pixelSize: 11
                                font.bold: true
                                font.family: "monospace"
                                Layout.fillWidth: true
                            }
                        }
                        Text {
                            visible: !!(root.upload.url && root.upload.url.length)
                            text: root.upload.url || ""
                            color: "#9dffb0"
                            font.pixelSize: 10
                            font.family: "monospace"
                            wrapMode: Text.WrapAnywhere
                            Layout.fillWidth: true
                        }
                        Text {
                            visible: !!(root.upload.error && root.upload.error.length)
                            text: root.upload.error || ""
                            color: "#ffaaaa"
                            font.pixelSize: 10
                            font.family: "monospace"
                            wrapMode: Text.Wrap
                            Layout.fillWidth: true
                        }
                    }
                }
            }

            Timer {
                id: dismiss
                interval: (root.preview.timeout || 10) * 1000
                running: previewWin.visible && root.upload.status !== "uploading"
                onTriggered: {
                    root.preview = ({})
                    root.upload = ({ status: "", url: "", error: "" })
                    Quickshell.execDetached(["rm", "-f", Quickshell.env("HOME") + "/.local/state/matrixshot/preview.json"])
                }
            }
            MouseArea {
                anchors.fill: parent
                hoverEnabled: true
                acceptedButtons: Qt.NoButton
                onEntered: dismiss.stop()
                onExited: {
                    if (previewWin.visible && root.upload.status !== "uploading")
                        dismiss.restart()
                }
                z: -1
            }
        }
    }

    Editor {
        imagePath: root.editPath
        onEditorClosed: {
            root.editPath = ""
            Quickshell.execDetached(["rm", "-f", Quickshell.env("HOME") + "/.local/state/matrixshot/edit.json"])
        }
        onEditorSaved: (path) => {
            Quickshell.execDetached(["bash", "-lc", "printf '%s\\n' screenshot \"" + path + "\" > \"$HOME/.local/state/matrixshot/last\""])
            root.editPath = ""
            root.preview = ({ path: path, name: path.split("/").pop(), dims: "", timeout: 10 })
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
                Text { text: "● REC"; color: "#ff3333"; font.bold: true; font.pixelSize: 14; font.family: "monospace" }
                Text { id: elapsed; color: "#eeeeee"; font.pixelSize: 14; font.family: "monospace"; text: "00:00:00" }
                MatrixIconButton {
                    label: "Stop"
                    Layout.preferredWidth: 72
                    Layout.fillWidth: false
                    onClicked: Quickshell.execDetached([root.bin, "record", "stop"])
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
