import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// The rail's Terminal view (docs: Design 11): shells the host runs in the thread's folder.
// The host keeps the screen and sends it as styled rows; this draws them and sends keys back,
// so every client looking at a shell sees the same one.
Item {
    id: term
    property var thread: null
    property var shells: []          // [{id, title, exited}]
    property string current: ""
    property var view: null          // {id, title, screen, exited}
    property string mono: ""
    signal start(int cols, int rows)
    signal open(string id)
    signal input(string id, string data)
    signal resize(string id, int cols, int rows)

    function shown(v) { if (v.id === current) view = v }

    FontMetrics { id: fm; font.family: term.mono; font.pixelSize: 12 }
    // Measured again whenever the font changes (it loads after this item is made).
    readonly property real cw: { void fm.font.family; return fm.advanceWidth("M") }
    readonly property real lh: Math.ceil(fm.height * 1.15)
    readonly property int fitCols: Math.max(20, Math.floor((screenArea.width - 20) / cw))
    readonly property int fitRows: Math.max(5, Math.floor((screenArea.height - 16) / lh))

    // Keeps the shell the size of the panel (the last client to resize wins).
    Timer {
        id: fit
        interval: 300
        onTriggered: if (term.current !== "" && term.view && (term.view.screen.cols !== term.fitCols || term.view.screen.rows !== term.fitRows))
            term.resize(term.current, term.fitCols, term.fitRows)
    }
    onFitColsChanged: fit.restart()
    onFitRowsChanged: fit.restart()
    onViewChanged: fit.restart()

    function esc(s) { return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/ /g, "&nbsp;") }
    function html(line) {
        let out = ""
        for (const r of line) {
            let style = ""
            if (r.fg) style += "color:" + r.fg + ";"
            if (r.bg) style += "background-color:" + r.bg + ";"
            let t = term.esc(r.t)
            if (r.b) t = "<b>" + t + "</b>"
            if (r.i) t = "<i>" + t + "</i>"
            if (r.u) t = "<u>" + t + "</u>"
            out += style ? "<span style=\"" + style + "\">" + t + "</span>" : t
        }
        return out
    }
    // The bytes a terminal sends for a key.
    function bytes(e) {
        const ctrl = e.modifiers & Qt.ControlModifier
        if (ctrl && e.key >= Qt.Key_A && e.key <= Qt.Key_Z) return String.fromCharCode(e.key - Qt.Key_A + 1)
        const keys = {}
        keys[Qt.Key_Return] = "\r"; keys[Qt.Key_Enter] = "\r"; keys[Qt.Key_Backspace] = "\x7f"; keys[Qt.Key_Tab] = "\t"
        keys[Qt.Key_Escape] = "\x1b"; keys[Qt.Key_Up] = "\x1b[A"; keys[Qt.Key_Down] = "\x1b[B"; keys[Qt.Key_Right] = "\x1b[C"
        keys[Qt.Key_Left] = "\x1b[D"; keys[Qt.Key_Home] = "\x1b[H"; keys[Qt.Key_End] = "\x1b[F"; keys[Qt.Key_Delete] = "\x1b[3~"
        keys[Qt.Key_PageUp] = "\x1b[5~"; keys[Qt.Key_PageDown] = "\x1b[6~"
        return keys[e.key] !== undefined ? keys[e.key] : e.text
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // The thread's shells (scrolling sideways when there are many), and "+" for another.
        RowLayout {
            Layout.fillWidth: true
            Layout.margins: 8
            spacing: 4
            ListView {
                Layout.fillWidth: true
                Layout.preferredHeight: 24
                orientation: ListView.Horizontal
                spacing: 4
                clip: true
                model: term.shells
                delegate: Btn {
                    required property var modelData
                    required property int index
                    kind: "quiet"; small: true
                    icon: "shell"
                    chosen: term.current === modelData.id
                    text: (term.shells.length > 1 ? "Shell " + (index + 1) : modelData.title) + (modelData.exited ? " (ended)" : "")
                    width: implicitWidth + 12
                    onClicked: { term.current = modelData.id; term.open(modelData.id) }
                }
            }
            Btn { kind: "icon"; icon: "plus"; small: true; enabled: term.thread !== null; onClicked: term.start(term.fitCols, term.fitRows) }
        }
        Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: theme.line }

        Rectangle {
            id: screenArea
            Layout.fillWidth: true
            Layout.fillHeight: true
            color: theme.app
            clip: true
            focus: true
            activeFocusOnTab: true
            Keys.onPressed: e => {
                if (term.current === "" || !term.view || term.view.exited) return
                const b = term.bytes(e)
                if (b !== "") { term.input(term.current, b); e.accepted = true }
            }
            MouseArea { anchors.fill: parent; onClicked: screenArea.forceActiveFocus() }

            Column {
                x: 10; y: 8
                Repeater {
                    model: term.view ? term.view.screen.lines : []
                    // A fixed slot per row: an empty row still takes its line.
                    delegate: Item {
                        required property var modelData
                        width: 1
                        height: term.lh
                        Text {
                            text: term.html(parent.modelData)
                            textFormat: Text.RichText
                            color: theme.text
                            font.family: term.mono
                            font.pixelSize: 12
                        }
                    }
                }
            }
            Rectangle {
                visible: term.view !== null && !term.view.exited
                x: 10 + (term.view ? term.view.screen.cursor[1] : 0) * term.cw
                y: 8 + (term.view ? term.view.screen.cursor[0] : 0) * term.lh
                width: term.cw; height: term.lh
                color: screenArea.activeFocus ? theme.text : "transparent"
                border.color: theme.text2
                opacity: 0.8
            }

            ColumnLayout {
                visible: term.current === ""
                anchors.centerIn: parent
                spacing: 10
                Icon { Layout.alignment: Qt.AlignHCenter; name: "terminal"; size: 30; stroke: 1.5; color: theme.text3 }
                Text { Layout.alignment: Qt.AlignHCenter; text: term.thread ? "A shell in this thread's folder, on the host." : "Open a thread first."; color: theme.text2 }
                Btn { Layout.alignment: Qt.AlignHCenter; visible: term.thread !== null; kind: "primary"; icon: "plus"; text: "New shell"; onClicked: term.start(term.fitCols, term.fitRows) }
            }
        }

        Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: theme.line }
        RowLayout {
            Layout.fillWidth: true
            Layout.preferredHeight: 30
            Layout.leftMargin: 12
            Layout.rightMargin: 12
            Text { Layout.fillWidth: true; elide: Text.ElideMiddle; text: term.thread ? "On this computer · " + term.thread.cwd : ""; color: theme.text3; font.pixelSize: theme.tiny }
            Text {
                Layout.alignment: Qt.AlignRight
                text: term.view ? term.view.title + " · " + term.view.screen.cols + "×" + term.view.screen.rows + (term.view.exited ? " · ended" : "") : ""
                color: theme.text3; font.pixelSize: theme.tiny
            }
        }
    }
}
