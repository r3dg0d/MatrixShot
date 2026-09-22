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
    color: "#cc000000"
    anchors {
        top: true
        left: true
        right: true
        bottom: true
    }

    property string tool: "pen" // pen | rect | arrow | highlight | text
    property color ink: "#00ff00"
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
            ctx.font = "bold 22px sans-serif"
            ctx.fillText(s.text || "", s.x, s.y)
        }
        ctx.restore()
    }

    Rectangle {
        id: chrome
        anchors.centerIn: parent
        width: Math.min(parent.width - 48, 1100)
        height: Math.min(parent.height - 48, 820)
        radius: 14
        color: "#121212"
        border.color: "#00ff00"
        border.width: 1

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 12
            spacing: 10

            RowLayout {
                Layout.fillWidth: true
                spacing: 8
                Text {
                    text: "MatrixShot Edit"
                    color: "#00ff00"
                    font.bold: true
                    font.pixelSize: 16
                }
                Item { Layout.fillWidth: true }
                Text {
                    text: imagePath.split("/").pop()
                    color: "#888"
                    elide: Text.ElideMiddle
                    Layout.maximumWidth: 360
                }
                Button { text: "Close"; onClicked: editorWin.editorClosed() }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                Button { text: "Pen"; highlighted: tool === "pen"; onClicked: tool = "pen" }
                Button { text: "Highlight"; highlighted: tool === "highlight"; onClicked: tool = "highlight" }
                Button { text: "Rect"; highlighted: tool === "rect"; onClicked: tool = "rect" }
                Button { text: "Arrow"; highlighted: tool === "arrow"; onClicked: tool = "arrow" }
                Button { text: "Text"; highlighted: tool === "text"; onClicked: tool = "text" }

                Rectangle { width: 1; height: 22; color: "#333" }

                Repeater {
                    model: ["#00ff00", "#ffffff", "#ff3333", "#33aaff", "#000000", "#ffff00"]
                    delegate: Rectangle {
                        width: 22; height: 22; radius: 4
                        color: modelData
                        border.color: ink === modelData ? "#fff" : "#444"
                        border.width: ink === modelData ? 2 : 1
                        MouseArea { anchors.fill: parent; onClicked: ink = modelData }
                    }
                }

                Rectangle { width: 1; height: 22; color: "#333" }
                Button { text: "Undo"; onClicked: undo() }
                Button { text: "Redo"; onClicked: redo() }
                Button { text: "Clear"; onClicked: clearAll() }
                Item { Layout.fillWidth: true }
                Button {
                    text: "Copy"
                    onClicked: stage.grabToImage(function (result) {
                        const tmp = Quickshell.env("HOME") + "/.local/state/matrixshot/edit-clipboard.png"
                        result.saveToFile(tmp)
                        Quickshell.execDetached(["wl-copy", "-t", "image/png"], { stdinFile: tmp })
                        // wl-copy needs file via shell
                        Quickshell.execDetached(["bash", "-lc", "wl-copy -t image/png < " + JSON.stringify(tmp)])
                    })
                }
                Button {
                    text: "Save"
                    highlighted: true
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

            // Image + annotation stage
            Item {
                id: stage
                Layout.fillWidth: true
                Layout.fillHeight: true
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

                // Map mouse into image coordinates
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
                        width: 280
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

            Text {
                text: "Esc closes · Enter commits text · Save writes *-edited.png and copies to clipboard"
                color: "#666"
                font.pixelSize: 11
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
