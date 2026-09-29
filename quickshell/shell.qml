import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland
import Quickshell.Io

Scope {
    id: root

    MatrixTheme { id: T }

    property var preview: ({})
    property var recording: ({ active: false, startedAt: 0, path: "" })
    property var upload: ({ status: "", url: "", error: "" })
    property var chooser: ({})
    property string editPath: ""
    property int recFps: 60
    property string recAudio: "desktop"
    property string recOutDir: Quickshell.env("HOME") + "/Videos/MatrixShot"
    property var audioDevices: []

    readonly property string bin: {
        const homeBin = Quickshell.env("HOME") + "/.local/bin/matrixshot"
        return homeBin
    }
    readonly property string iconDir: Qt.resolvedUrl("./icons/")
    readonly property string stateDir: Quickshell.env("HOME") + "/.local/state/matrixshot"
    readonly property bool chooserOpen: !!(root.chooser && root.chooser.geometry && root.chooser.phase)
    readonly property bool choosePhase: root.chooserOpen && root.chooser.phase === "choose"
    readonly property bool recordConfigPhase: root.chooserOpen && root.chooser.phase === "record-config"

    FileView {
        id: previewFile
        path: root.stateDir + "/preview.json"
        watchChanges: true
        onFileChanged: previewFile.reload()
        onLoaded: {
            try { root.preview = JSON.parse(previewFile.text()) }
            catch (e) { root.preview = ({}) }
        }
    }

    FileView {
        id: recordFile
        path: root.stateDir + "/recording.json"
        watchChanges: true
        onFileChanged: recordFile.reload()
        onLoaded: {
            try { root.recording = JSON.parse(recordFile.text()) }
            catch (e) { root.recording = ({ active: false }) }
        }
    }

    FileView {
        id: uploadFile
        path: root.stateDir + "/upload.json"
        watchChanges: true
        onFileChanged: uploadFile.reload()
        onLoaded: {
            try { root.upload = JSON.parse(uploadFile.text()) }
            catch (e) { root.upload = ({ status: "", url: "", error: "" }) }
        }
    }

    FileView {
        id: editRequest
        path: root.stateDir + "/edit.json"
        watchChanges: true
        onFileChanged: editRequest.reload()
        onLoaded: {
            try {
                const j = JSON.parse(editRequest.text())
                if (j && j.path) root.editPath = j.path
            } catch (e) {}
        }
    }

    FileView {
        id: chooserFile
        path: root.stateDir + "/chooser.json"
        watchChanges: true
        onFileChanged: chooserFile.reload()
        onLoaded: {
            try {
                const j = JSON.parse(chooserFile.text())
                root.chooser = j && j.geometry ? j : ({})
                if (root.chooser.phase === "record-config")
                    root.loadAudioDevices()
            } catch (e) { root.chooser = ({}) }
        }
    }

    Process {
        id: audioProc
        command: [root.bin, "record", "list-audio"]
        stdout: StdioCollector {
            onStreamFinished: {
                const lines = text.trim().split("\n").filter(l => l.length > 0)
                const devices = []
                for (let i = 0; i < lines.length; i++) {
                    const parts = lines[i].split("|")
                    const id = parts[0] || ""
                    const label = parts.slice(1).join("|") || id
                    if (id) devices.push({ id: id, label: label })
                }
                root.audioDevices = devices
            }
        }
    }

    function loadAudioDevices() {
        audioProc.running = false
        audioProc.running = true
    }

    function dismissChooser() {
        root.chooser = ({})
        Quickshell.execDetached(["rm", "-f", root.stateDir + "/chooser.json"])
    }

    function setChooserPhase(phase) {
        if (!root.chooser.geometry) return
        const body = JSON.stringify({
            geometry: root.chooser.geometry,
            phase: phase,
            ts: Math.floor(Date.now() / 1000)
        })
        Quickshell.execDetached(["bash", "-lc",
            "printf '%s\\n' " + JSON.stringify(body) + " > \"" + root.stateDir + "/chooser.json\""
        ])
        root.chooser = ({ geometry: root.chooser.geometry, phase: phase })
        if (phase === "record-config") root.loadAudioDevices()
    }

    function doScreenshot() {
        const g = root.chooser.geometry
        if (!g) return
        Quickshell.execDetached([root.bin, "region", "--geometry", g])
        root.chooser = ({})
    }

    function startRecording() {
        const g = root.chooser.geometry
        if (!g) return
        const args = [root.bin, "record", "region",
            "--geometry", g,
            "--fps", String(root.recFps),
            "--audio", root.recAudio,
            "--output-dir", root.recOutDir]
        Quickshell.execDetached(args)
        root.chooser = ({})
    }

    function openEditor(path) {
        dismiss.stop()
        root.preview = ({})
        root.editPath = path
        Quickshell.execDetached(["rm", "-f", root.stateDir + "/preview.json"])
    }

    component MatrixIconButton: Rectangle {
        id: btn
        property string label: ""
        property string iconName: ""
        property bool busy: false
        property bool primary: false
        property bool selected: false
        signal clicked()

        Layout.fillWidth: true
        Layout.preferredHeight: 36
        radius: 8
        color: {
            if (selected) return "#1a3d1a"
            if (ma.containsMouse) return primary ? "#1a3d1a" : "#1a1a1a"
            return primary ? "#122612" : T.muteBg
        }
        border.color: (primary || selected) ? T.phosphor : (ma.containsMouse ? T.phosphorDim : T.border)
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
                color: T.phosphorText
                font.pixelSize: 12
                font.bold: true
                font.family: T.mono
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

    // ---- Post-region chooser: Screenshot | Screen Record ----
    PanelWindow {
        id: chooserWin
        visible: root.choosePhase && root.editPath.length === 0
        screen: Quickshell.screens.length > 0 ? Quickshell.screens[0] : null
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.keyboardFocus: WlrKeyboardFocus.OnDemand
        exclusiveZone: 0
        color: "transparent"
        anchors { top: true; right: true }
        margins { top: 12; right: 12 }
        implicitWidth: 300
        implicitHeight: chooseCard.implicitHeight

        Rectangle {
            id: chooseCard
            anchors.fill: parent
            radius: 12
            color: T.panel
            border.color: T.phosphor
            border.width: 1
            implicitHeight: chooseCol.implicitHeight + 24

            ColumnLayout {
                id: chooseCol
                anchors.fill: parent
                anchors.margins: 12
                spacing: 10

                RowLayout {
                    Layout.fillWidth: true
                    Text {
                        text: "MATRIXSHOT"
                        color: T.phosphor
                        font.pixelSize: 12
                        font.bold: true
                        font.letterSpacing: 1.5
                        font.family: T.mono
                        Layout.fillWidth: true
                    }
                    Rectangle {
                        width: 28; height: 28; radius: 6
                        color: chooseCloseMa.containsMouse ? T.criticalBg : T.muteBg
                        border.color: chooseCloseMa.containsMouse ? T.critical : T.borderHover
                        border.width: 1
                        Image {
                            anchors.centerIn: parent
                            source: root.iconDir + "close.svg"
                            sourceSize.width: 14; sourceSize.height: 14
                            width: 14; height: 14
                        }
                        MouseArea {
                            id: chooseCloseMa
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.dismissChooser()
                        }
                    }
                }

                Text {
                    text: "Region  " + (root.chooser.geometry || "")
                    color: T.phosphorSoft
                    font.pixelSize: 10
                    font.family: T.mono
                    elide: Text.ElideMiddle
                    Layout.fillWidth: true
                }

                MatrixIconButton {
                    label: "Screenshot"
                    iconName: "screenshot"
                    primary: true
                    Layout.preferredHeight: 42
                    onClicked: root.doScreenshot()
                }
                MatrixIconButton {
                    label: "Screen Record"
                    iconName: "record"
                    Layout.preferredHeight: 42
                    onClicked: root.setChooserPhase("record-config")
                }
            }
        }
    }

    // ---- Recording config (after Screen Record) ----
    PanelWindow {
        id: recCfgWin
        visible: root.recordConfigPhase && root.editPath.length === 0
        screen: Quickshell.screens.length > 0 ? Quickshell.screens[0] : null
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.keyboardFocus: WlrKeyboardFocus.OnDemand
        exclusiveZone: 0
        color: "transparent"
        anchors { top: true; right: true }
        margins { top: 12; right: 12 }
        implicitWidth: 340
        implicitHeight: recCfgCard.implicitHeight

        Rectangle {
            id: recCfgCard
            anchors.fill: parent
            radius: 12
            color: T.panel
            border.color: T.phosphor
            border.width: 1
            implicitHeight: recCfgCol.implicitHeight + 24

            ColumnLayout {
                id: recCfgCol
                anchors.fill: parent
                anchors.margins: 12
                spacing: 8

                RowLayout {
                    Layout.fillWidth: true
                    Text {
                        text: "RECORD CONFIG"
                        color: T.phosphor
                        font.pixelSize: 12
                        font.bold: true
                        font.letterSpacing: 1.2
                        font.family: T.mono
                        Layout.fillWidth: true
                    }
                    Rectangle {
                        width: 28; height: 28; radius: 6
                        color: recCfgCloseMa.containsMouse ? T.criticalBg : T.muteBg
                        border.color: recCfgCloseMa.containsMouse ? T.critical : T.borderHover
                        border.width: 1
                        Image {
                            anchors.centerIn: parent
                            source: root.iconDir + "close.svg"
                            sourceSize.width: 14; sourceSize.height: 14
                            width: 14; height: 14
                        }
                        MouseArea {
                            id: recCfgCloseMa
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.dismissChooser()
                        }
                    }
                }

                Text {
                    text: "Region  " + (root.chooser.geometry || "")
                    color: T.phosphorSoft
                    font.pixelSize: 10
                    font.family: T.mono
                    elide: Text.ElideMiddle
                    Layout.fillWidth: true
                }

                Text {
                    text: "FPS"
                    color: T.phosphor
                    font.pixelSize: 11
                    font.bold: true
                    font.family: T.mono
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 6
                    Repeater {
                        model: [30, 60, 120]
                        MatrixIconButton {
                            required property int modelData
                            label: String(modelData)
                            selected: root.recFps === modelData
                            Layout.preferredHeight: 32
                            onClicked: root.recFps = modelData
                        }
                    }
                }

                Text {
                    text: "AUDIO"
                    color: T.phosphor
                    font.pixelSize: 11
                    font.bold: true
                    font.family: T.mono
                }
                GridLayout {
                    Layout.fillWidth: true
                    columns: 2
                    rowSpacing: 6
                    columnSpacing: 6
                    Repeater {
                        model: [
                            { id: "none", label: "None" },
                            { id: "desktop", label: "Desktop" },
                            { id: "mic", label: "Mic" },
                            { id: "both", label: "Both" }
                        ]
                        MatrixIconButton {
                            required property var modelData
                            label: modelData.label
                            selected: root.recAudio === modelData.id
                            Layout.preferredHeight: 32
                            onClicked: root.recAudio = modelData.id
                        }
                    }
                }

                Text {
                    visible: root.audioDevices.length > 0
                    text: "DEVICES (info)"
                    color: T.phosphor
                    font.pixelSize: 11
                    font.bold: true
                    font.family: T.mono
                }
                Text {
                    visible: root.audioDevices.length > 0
                    text: {
                        const ids = ["default_output", "default_input"]
                        const lines = []
                        for (let i = 0; i < root.audioDevices.length; i++) {
                            const d = root.audioDevices[i]
                            if (ids.indexOf(d.id) >= 0)
                                lines.push(d.id + " — " + d.label)
                        }
                        // also show first few extras
                        let extra = 0
                        for (let i = 0; i < root.audioDevices.length && extra < 3; i++) {
                            const d = root.audioDevices[i]
                            if (ids.indexOf(d.id) < 0) {
                                lines.push(d.id.split(".").slice(-1)[0] + " — " + d.label)
                                extra++
                            }
                        }
                        return lines.join("\n") || "default_output / default_input"
                    }
                    color: "#6a9a6a"
                    font.pixelSize: 9
                    font.family: T.mono
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                }

                Text {
                    text: "OUTPUT"
                    color: T.phosphor
                    font.pixelSize: 11
                    font.bold: true
                    font.family: T.mono
                }
                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 32
                    radius: 6
                    color: "#0a0a0a"
                    border.color: T.border
                    border.width: 1
                    TextInput {
                        anchors.fill: parent
                        anchors.margins: 8
                        text: root.recOutDir
                        color: T.phosphorText
                        font.pixelSize: 11
                        font.family: T.mono
                        clip: true
                        selectByMouse: true
                        onTextChanged: root.recOutDir = text
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 6
                    MatrixIconButton {
                        label: "Back"
                        Layout.preferredHeight: 38
                        onClicked: root.setChooserPhase("choose")
                    }
                    MatrixIconButton {
                        label: "Start"
                        iconName: "record"
                        primary: true
                        Layout.preferredHeight: 38
                        onClicked: root.startRecording()
                    }
                }
            }
        }
    }

    // Screenshot preview card — top-right
    PanelWindow {
        id: previewWin
        visible: !!(root.preview && root.preview.path) && root.editPath.length === 0 && !root.chooserOpen
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
            color: T.panel
            border.color: T.phosphor
            border.width: 1
            implicitHeight: col.implicitHeight + 24

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
                        color: T.phosphor
                        font.pixelSize: 12
                        font.bold: true
                        font.letterSpacing: 1.5
                        font.family: T.mono
                        Layout.fillWidth: true
                    }
                    Rectangle {
                        width: 28
                        height: 28
                        radius: 6
                        color: closeMa.containsMouse ? T.criticalBg : T.muteBg
                        border.color: closeMa.containsMouse ? T.critical : T.borderHover
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
                                Quickshell.execDetached(["rm", "-f", root.stateDir + "/preview.json"])
                                Quickshell.execDetached(["rm", "-f", root.stateDir + "/upload.json"])
                            }
                        }
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 148
                    radius: 8
                    color: "#0a0a0a"
                    border.color: T.borderStrong
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
                    color: T.phosphorSoft
                    font.pixelSize: 11
                    font.family: T.mono
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
                    border.color: root.upload.status === "error" ? T.critical : T.phosphor
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
                                color: root.upload.status === "error" ? T.criticalSoft : T.phosphor
                                font.pixelSize: 11
                                font.bold: true
                                font.family: T.mono
                                Layout.fillWidth: true
                            }
                        }
                        Text {
                            visible: !!(root.upload.url && root.upload.url.length)
                            text: root.upload.url || ""
                            color: "#9dffb0"
                            font.pixelSize: 10
                            font.family: T.mono
                            wrapMode: Text.WrapAnywhere
                            Layout.fillWidth: true
                        }
                        Text {
                            visible: !!(root.upload.error && root.upload.error.length)
                            text: root.upload.error || ""
                            color: "#ffaaaa"
                            font.pixelSize: 10
                            font.family: T.mono
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
                    Quickshell.execDetached(["rm", "-f", root.stateDir + "/preview.json"])
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
            Quickshell.execDetached(["rm", "-f", root.stateDir + "/edit.json"])
        }
        onEditorSaved: (path) => {
            Quickshell.execDetached(["bash", "-lc", "printf '%s\\n' screenshot \"" + path + "\" > \"" + root.stateDir + "/last\""])
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
                Text { text: "● REC"; color: "#ff3333"; font.bold: true; font.pixelSize: 14; font.family: T.mono }
                Text { id: elapsed; color: "#eeeeee"; font.pixelSize: 14; font.family: T.mono; text: "00:00:00" }
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
