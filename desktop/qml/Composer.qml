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
    readonly property bool empty: input.text.trim() === ""
    implicitHeight: col.implicitHeight + 24
    radius: theme.radius
    color: theme.card
    border.color: input.activeFocus ? "#3A3A3A" : theme.line

    function submit() {
        const t = input.text.trim()
        if (t === "") return
        box.send(t)
        input.text = ""
    }

    ColumnLayout {
        id: col
        anchors.fill: parent
        anchors.margins: 12
        spacing: 10
        TextArea {
            id: input
            Layout.fillWidth: true
            Layout.maximumHeight: 200
            leftPadding: 4
            topPadding: 2
            placeholderText: box.thread ? (box.thread.parent_id ? "Message this team thread…" : "Message the main thread…") : ""
            placeholderTextColor: theme.text3
            color: theme.text
            font.pixelSize: theme.size
            wrapMode: TextArea.Wrap
            background: null
            Keys.onReturnPressed: event => {
                if (event.modifiers & Qt.ShiftModifier) event.accepted = false
                else box.submit()
            }
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            // The chip opens the picker: team, model, thinking, permissions.
            Rectangle {
                visible: box.thread !== null
                implicitWidth: chip.implicitWidth + 16
                implicitHeight: 26
                radius: theme.radiusSmall
                color: chipArea.containsMouse ? theme.selection : theme.field
                Row {
                    id: chip
                    anchors.centerIn: parent
                    spacing: 7
                    TeamBadge { anchors.verticalCenter: parent.verticalCenter; team: box.team; size: 16 }
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        leftPadding: 3
                        text: box.thread ? (box.thread.model || "default model") : ""
                        color: theme.text; font.pixelSize: theme.small
                    }
                    Rectangle { anchors.verticalCenter: parent.verticalCenter; width: 1; height: 12; color: "#3A3A3A" }
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: box.thread ? theme.freedomLabel(box.thread.freedom) : ""
                        color: box.thread && box.thread.freedom === "full" ? theme.amber : theme.text2
                        font.pixelSize: theme.small
                    }
                    Icon { anchors.verticalCenter: parent.verticalCenter; name: "up"; size: 12; stroke: 2; color: theme.text3 }
                }
                MouseArea { id: chipArea; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: box.openPicker() }
            }
            Item { Layout.fillWidth: true }
            Text { visible: !box.empty; text: "Enter to send · Shift+Enter for a new line"; color: theme.text3; font.pixelSize: theme.tiny }
            Rectangle {
                width: 30; height: 30; radius: 15
                color: box.empty ? theme.selection : theme.text
                Icon { anchors.centerIn: parent; name: "send"; size: 16; stroke: 2.2; color: box.empty ? theme.text3 : theme.onLight }
                MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: box.submit() }
            }
        }
    }
}
