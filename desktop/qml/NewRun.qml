import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// A new run: its main thread, folder, team and freedom (Scope §3.1; the screen isn't designed
// yet, so this is the plain version).
Rectangle {
    id: form
    property var teams: []
    signal create(var params)
    signal close()

    color: theme.raised
    radius: theme.radius
    border.color: theme.line
    implicitHeight: col.implicitHeight + 28
    implicitWidth: 460

    ColumnLayout {
        id: col
        x: 14; y: 14
        width: parent.width - 28
        spacing: 8
        Text { text: "New run"; color: theme.text; font.weight: Font.DemiBold }
        Label { text: "Title" }
        Field { id: title; placeholderText: "What this run is for" }
        Label { text: "Folder (a git repository, so teams can work on their own branch)" }
        Field { id: folder; placeholderText: "C:\\path\\to\\project" }
        Label { text: "Main team" }
        ComboBox { id: team; Layout.fillWidth: true; model: form.teams.map(t => t.name) }
        Label { text: "Freedom" }
        ComboBox { id: freedom; Layout.fillWidth: true; model: ["ask", "sandbox", "plan", "full"] }
        RowLayout {
            Layout.fillWidth: true
            Layout.topMargin: 6
            Item { Layout.fillWidth: true }
            Button { text: "Cancel"; flat: true; onClicked: form.close() }
            Button {
                text: "Create"
                enabled: title.text.trim() !== "" && folder.text.trim() !== "" && team.currentText !== ""
                onClicked: form.create({ title: title.text.trim(), team: team.currentText, cwd: folder.text.trim(), model: null, effort: null, permission_mode: freedom.currentText })
            }
        }
    }

    component Label: Text { color: theme.text3; font.pixelSize: theme.small; Layout.topMargin: 4 }
    component Field: TextField {
        Layout.fillWidth: true
        color: theme.text
        placeholderTextColor: theme.text3
        background: Rectangle { radius: 6; color: theme.field; border.color: parent.activeFocus ? theme.text3 : theme.line }
    }
}
