import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// The message box, with the thread's chip (team · model │ freedom) under it (docs: Design 01, 25).
Rectangle {
    id: box
    property var thread: null
    property var teams: []
    signal send(string text)
    signal openPicker()

    readonly property var team: thread ? teams.find(t => t.id === thread.team_id) || null : null
    implicitHeight: col.implicitHeight + 20
    radius: theme.radius
    color: theme.raised
    border.color: input.activeFocus ? theme.text3 : theme.line

    function submit() {
        const t = input.text.trim()
        if (t === "") return
        box.send(t)
        input.text = ""
    }

    ColumnLayout {
        id: col
        anchors.fill: parent
        anchors.margins: 10
        spacing: 8
        TextArea {
            id: input
            Layout.fillWidth: true
            placeholderText: box.thread ? (box.thread.parent_id ? "Message this team thread…" : "Message the main thread…") : ""
            placeholderTextColor: theme.text3
            color: theme.text
            wrapMode: TextArea.Wrap
            background: null
            Keys.onReturnPressed: event => {
                if (event.modifiers & Qt.ShiftModifier) event.accepted = false
                else box.submit()
            }
        }
        RowLayout {
            Layout.fillWidth: true
            Rectangle {
                implicitWidth: chip.implicitWidth + 16
                implicitHeight: 24
                radius: 6
                color: theme.field
                Text {
                    id: chip
                    anchors.centerIn: parent
                    color: theme.text2
                    font.pixelSize: theme.small
                    text: !box.thread ? "" : (box.team ? box.team.name : "no team") + " · " + (box.thread.model || "default model") + "  │  " + box.thread.freedom + "  ⌃"
                }
                MouseArea { anchors.fill: parent; onClicked: box.openPicker() }
            }
            Item { Layout.fillWidth: true }
            Rectangle {
                width: 28; height: 28; radius: 14
                color: input.text.trim() === "" ? theme.field : theme.text
                Text { anchors.centerIn: parent; text: "↑"; color: input.text.trim() === "" ? theme.text3 : "#111"; font.weight: Font.Bold }
                MouseArea { anchors.fill: parent; onClicked: box.submit() }
            }
        }
    }
}
