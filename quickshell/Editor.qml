import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland

PanelWindow {
    id: editorWin

    required property string imagePath
    signal editorClosed()
    signal editorSaved(string path)

    visible: imagePath.length > 0
    screen: Quickshell.screens.length > 0 ? Quickshell.screens[0] : null
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.OnDemand
    exclusiveZone: 0
    color: "#cc050805"
    anchors {
        top: true
        left: true
        right: true
        bottom: true
    }

    readonly property string iconDir: Qt.resolvedUrl("./icons/")

    property string tool: "pen" // pen | rect | arrow | highlight | text
    property color ink: "#00ff66"
    property real penWidth: 3
    property var strokes: []
    property var redoStack: []
    property var draft: null

    function pushStroke(s) {
        strokes = strokes.concat([s])
        redoStack = []
        overlay.requestPaint()
    }

    function undo() {
        if (strokes.length === 0) return
        const next = strokes.slice()
        const last = next.pop()
        strokes = next
        redoStack = redoStack.concat([last])
        overlay.requestPaint()
    }

    function redo() {
        if (redoStack.length === 0) return
        const next = redoStack.slice()
        const s = next.pop()
        redoStack = next
        strokes = strokes.concat([s])
        overlay.requestPaint()
    }

    function clearAll() {
        strokes = []
        redoStack = []
        draft = null
        overlay.requestPaint()
    }

    function drawStroke(ctx, s) {
        ctx.save()
        ctx.lineCap = "round"
        ctx.lineJoin = "round"
        if (s.tool === "highlight") {
            ctx.globalAlpha = 0.35
            ctx.strokeStyle = s.color
            ctx.lineWidth = s.width * 4
        } else {
            ctx.globalAlpha = 1.0
            ctx.strokeStyle = s.color
            ctx.lineWidth = s.width
        }
        if (s.tool === "pen" || s.tool === "highlight") {
            if (!s.points || s.points.length < 2) { ctx.restore(); return }
            ctx.beginPath()
            ctx.moveTo(s.points[0].x, s.points[0].y)
            for (let i = 1; i < s.points.length; i++)
                ctx.lineTo(s.points[i].x, s.points[i].y)
            ctx.stroke()
        } else if (s.tool === "rect") {
            ctx.strokeRect(s.x, s.y, s.w, s.h)
        } else if (s.tool === "arrow") {
            const x1 = s.x1, y1 = s.y1, x2 = s.x2, y2 = s.y2
            ctx.beginPath()
            ctx.moveTo(x1, y1)
            ctx.lineTo(x2, y2)
            ctx.stroke()
            const angle = Math.atan2(y2 - y1, x2 - x1)
            const head = 14 + s.width
            ctx.beginPath()
            ctx.moveTo(x2, y2)
            ctx.lineTo(x2 - head * Math.cos(angle - 0.4), y2 - head * Math.sin(angle - 0.4))
            ctx.lineTo(x2 - head * Math.cos(angle + 0.4), y2 - head * Math.sin(angle + 0.4))
            ctx.closePath()
            ctx.fillStyle = s.color
            ctx.fill()
        } else if (s.tool === "text") {
            ctx.globalAlpha = 1.0
            ctx.fillStyle = s.color
            ctx.font = "bold 22px monospace"
            ctx.fillText(s.text || "", s.x, s.y)
        }
        ctx.restore()
    }

    component ToolBtn: Rectangle {
        id: tb
        property string label: ""
        property string iconName: ""
        property bool active: false
        property bool primary: false
        property bool danger: false
        signal clicked()

        Layout.preferredHeight: 34
        Layout.preferredWidth: Math.max(36, implicitWidth)
        implicitWidth: row.implicitWidth + 18
        radius: 8
        color: {
            if (ma.pressed) return primary ? "#1a4a1a" : "#1a1a1a"
            if (active || ma.containsMouse) return primary ? "#1a3d1a" : "#1a1a1a"
            return primary ? "#122612" : "#101410"
        }
        border.width: 1
        border.color: {
            if (danger && ma.containsMouse) return "#ff5555"
            if (primary || active) return "#00ff66"
            if (ma.containsMouse) return "#00cc55"
            return "#1f5f1f"
        }

        Row {
            id: row
            anchors.centerIn: parent
            spacing: 6
            Image {
                visible: tb.iconName.length > 0
                anchors.verticalCenter: parent.verticalCenter
                source: editorWin.iconDir + tb.iconName + ".svg"
                sourceSize.width: 15
                sourceSize.height: 15
                width: 15
                height: 15
                fillMode: Image.PreserveAspectFit
            }
            Text {
                visible: tb.label.length > 0
                anchors.verticalCenter: parent.verticalCenter
                text: tb.label
                color: danger && ma.containsMouse ? "#ff8888" : "#b8ffb8"
                font.pixelSize: 11
                font.bold: true
                font.family: "monospace"
            }
        }

        MouseArea {
            id: ma
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: tb.clicked()
        }
    }

    Rectangle {
        id: chrome
        anchors.centerIn: parent
        width: Math.min(parent.width - 48, 1100)
        height: Math.min(parent.height - 48, 820)
        radius: 14
        color: "#f0080c08"
        border.color: "#00ff66"
        border.width: 1

        Rectangle {
            anchors.fill: parent
            anchors.margins: 1
            radius: 13
            color: "transparent"
            border.color: "#2200ff66"
            border.width: 1
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 12
            spacing: 10

            RowLayout {
                Layout.fillWidth: true
                spacing: 10
                Text {
                    text: "MATRIXSHOT EDIT"
                    color: "#00ff66"
                    font.bold: true
                    font.pixelSize: 13
                    font.letterSpacing: 1.4
                    font.family: "monospace"
                }
                Item { Layout.fillWidth: true }
                Text {
                    text: imagePath.split("/").pop()
                    color: "#7dff9a"
                    font.pixelSize: 11
                    font.family: "monospace"
                    elide: Text.ElideMiddle
                    Layout.maximumWidth: 360
                }
                ToolBtn {
                    iconName: "close"
                    label: "Close"
                    danger: true
                    onClicked: editorWin.editorClosed()
                }
            }

            // Tool strip
            Rectangle {
                Layout.fillWidth: true
                radius: 10
                color: "#0a0f0a"
                border.color: "#1a4a1a"
                border.width: 1
                implicitHeight: strip.implicitHeight + 16

                RowLayout {
                    id: strip
                    anchors.fill: parent
                    anchors.margins: 8
                    spacing: 6

                    ToolBtn { iconName: "pen"; label: "Pen"; active: tool === "pen"; onClicked: tool = "pen" }
                    ToolBtn { iconName: "highlight"; label: "Highlight"; active: tool === "highlight"; onClicked: tool = "highlight" }
                    ToolBtn { iconName: "rect"; label: "Rect"; active: tool === "rect"; onClicked: tool = "rect" }
                    ToolBtn { iconName: "arrow"; label: "Arrow"; active: tool === "arrow"; onClicked: tool = "arrow" }
                    ToolBtn { iconName: "text"; label: "Text"; active: tool === "text"; onClicked: tool = "text" }

                    Rectangle { Layout.preferredWidth: 1; Layout.preferredHeight: 22; color: "#1f5f1f" }

                    Repeater {
                        model: ["#00ff66", "#ffffff", "#ff3333", "#33aaff", "#000000", "#ffff00"]
                        delegate: Rectangle {
                            width: 24; height: 24; radius: 6
                            color: modelData
                            border.color: ink === modelData ? "#00ff66" : "#335533"
                            border.width: ink === modelData ? 2 : 1
                            Rectangle {
                                anchors.fill: parent
                                anchors.margins: -3
                                radius: 8
                                color: "transparent"
                                border.color: ink === modelData ? "#6600ff66" : "transparent"
                                border.width: 1
                                z: -1
                            }
                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onClicked: ink = modelData
                            }
                        }
                    }

                    Rectangle { Layout.preferredWidth: 1; Layout.preferredHeight: 22; color: "#1f5f1f" }

                    ToolBtn { iconName: "undo"; label: "Undo"; onClicked: undo() }
                    ToolBtn { iconName: "redo"; label: "Redo"; onClicked: redo() }
                    ToolBtn { iconName: "clear"; label: "Clear"; danger: true; onClicked: clearAll() }

                    Item { Layout.fillWidth: true }

                    ToolBtn {
                        iconName: "copy"
                        label: "Copy"
                        onClicked: stage.grabToImage(function (result) {
                            const tmp = Quickshell.env("HOME") + "/.local/state/matrixshot/edit-clipboard.png"
                            result.saveToFile(tmp)
                            Quickshell.execDetached(["bash", "-lc", "wl-copy -t image/png < " + JSON.stringify(tmp)])
                        })
                    }
                    ToolBtn {
                        iconName: "save"
                        label: "Save"
                        primary: true
                        onClicked: stage.grabToImage(function (result) {
                            const base = imagePath.replace(/\.png$/i, "")
                            const out = base + "-edited.png"
                            if (result.saveToFile(out)) {
                                editorWin.editorSaved(out)
                                Quickshell.execDetached(["bash", "-lc", "wl-copy -t image/png < " + JSON.stringify(out)])
                            }
                        })
                    }
                }
            }

            // Image + annotation stage
            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                radius: 10
                color: "#050805"
                border.color: "#1a4a1a"
                border.width: 1
                clip: true

                Item {
                    id: stage
                    anchors.fill: parent
                    anchors.margins: 6
                    clip: true

                    Image {
                        id: baseImg
                        anchors.centerIn: parent
                        width: Math.min(parent.width, sourceSize.width)
                        height: Math.min(parent.height, sourceSize.height)
                        fillMode: Image.PreserveAspectFit
                        asynchronous: false
                        source: imagePath ? ("file://" + imagePath) : ""
                        cache: false
                    }

                    Item {
                        id: paintArea
                        anchors.fill: baseImg

                        Canvas {
                            id: overlay
                            anchors.fill: parent
                            renderTarget: Canvas.Image
                            renderStrategy: Canvas.Cooperative

                            onPaint: {
                                const ctx = getContext("2d")
                                ctx.clearRect(0, 0, width, height)
                                for (let i = 0; i < strokes.length; i++)
                                    drawStroke(ctx, strokes[i])
                                if (draft)
                                    drawStroke(ctx, draft)
                            }
                        }

                        MouseArea {
                            id: mouse
                            anchors.fill: parent
                            acceptedButtons: Qt.LeftButton
                            hoverEnabled: true

                            property real sx: 0
                            property real sy: 0

                            onPressed: (mouse) => {
                                sx = mouse.x; sy = mouse.y
                                if (tool === "text") {
                                    textInput.x = mouse.x
                                    textInput.y = mouse.y - 12
                                    textInput.visible = true
                                    textInput.forceActiveFocus()
                                    textInput.text = ""
                                    return
                                }
                                if (tool === "pen" || tool === "highlight") {
                                    draft = { tool: tool, color: ink.toString(), width: penWidth, points: [{ x: mouse.x, y: mouse.y }] }
                                } else if (tool === "rect") {
                                    draft = { tool: "rect", color: ink.toString(), width: penWidth, x: mouse.x, y: mouse.y, w: 0, h: 0 }
                                } else if (tool === "arrow") {
                                    draft = { tool: "arrow", color: ink.toString(), width: penWidth, x1: mouse.x, y1: mouse.y, x2: mouse.x, y2: mouse.y }
                                }
                                overlay.requestPaint()
                            }
                            onPositionChanged: (mouse) => {
                                if (!pressed || !draft) return
                                if (draft.tool === "pen" || draft.tool === "highlight") {
                                    draft.points = draft.points.concat([{ x: mouse.x, y: mouse.y }])
                                    draft = Object.assign({}, draft)
                                } else if (draft.tool === "rect") {
                                    draft = Object.assign({}, draft, {
                                        x: Math.min(sx, mouse.x),
                                        y: Math.min(sy, mouse.y),
                                        w: Math.abs(mouse.x - sx),
                                        h: Math.abs(mouse.y - sy)
                                    })
                                } else if (draft.tool === "arrow") {
                                    draft = Object.assign({}, draft, { x2: mouse.x, y2: mouse.y })
                                }
                                overlay.requestPaint()
                            }
                            onReleased: {
                                if (draft && draft.tool !== "text") {
                                    pushStroke(draft)
                                    draft = null
                                }
                            }
                        }

                        TextInput {
                            id: textInput
                            visible: false
                            color: ink
                            font.pixelSize: 22
                            font.bold: true
                            font.family: "monospace"
                            width: 280
                            selectionColor: "#003300"
                            selectedTextColor: "#00ff66"
                            onAccepted: {
                                if (text.length > 0) {
                                    pushStroke({ tool: "text", color: ink.toString(), width: penWidth, x: x, y: y + 18, text: text })
                                }
                                visible = false
                                text = ""
                            }
                            Keys.onEscapePressed: {
                                visible = false
                                text = ""
                            }
                        }
                    }
                }
            }

            Text {
                text: "Esc closes · Enter commits text · Save writes *-edited.png and copies to clipboard"
                color: "#3d7a4a"
                font.pixelSize: 11
                font.family: "monospace"
                Layout.fillWidth: true
            }
        }

        Keys.onPressed: (event) => {
            if (event.key === Qt.Key_Escape) {
                if (textInput.visible) {
                    textInput.visible = false
                    event.accepted = true
                    return
                }
                editorWin.editorClosed()
                event.accepted = true
            } else if (event.key === Qt.Key_Z && (event.modifiers & Qt.ControlModifier)) {
                undo(); event.accepted = true
            }
        }
        focus: true
        Component.onCompleted: forceActiveFocus()
    }
}
