import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// A new run: its main thread, folder, team and freedom (Scope §3.1; the screen isn't designed
// yet, so it follows the picker's look).
Rectangle {
    id: form
    property var teams: []
    property string folder: ""
    property string team: teams.length ? teams[0].name : ""
    property string freedom: "ask"
    property string defaultFreedom: "ask"
    property string monoFamily: ""
    signal create(var params)
    signal close()

    readonly property var chosenTeam: teams.find(t => t.name === team) || null
    readonly property bool agy: chosenTeam !== null && chosenTeam.cli === "agy"
    readonly property bool ready: title.text.trim() !== "" && path.text.trim() !== "" && team !== ""
    onVisibleChanged: if (visible) { path.text = folder; freedom = defaultFreedom; title.forceActiveFocus() }

    color: theme.raised
    radius: theme.radius
    border.color: theme.line
    implicitHeight: col.implicitHeight
    implicitWidth: 480

    ColumnLayout {
        id: col
        width: parent.width
        spacing: 0
        RowLayout {
            Layout.fillWidth: true
            Layout.preferredHeight: 50
            Layout.leftMargin: theme.pad
            Layout.rightMargin: 8
            spacing: 8
            Icon { name: "plus"; size: 16; color: theme.text2 }
            Text { text: "New run"; color: theme.text; font.weight: Font.DemiBold; Layout.fillWidth: true }
            Btn { kind: "icon"; icon: "close"; onClicked: form.close() }
        }
        Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: theme.line }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.margins: theme.pad
            spacing: 6
            Label { text: "What this run is for" }
            Field { id: title; placeholderText: "Fix the flaky upload test" }
            Label { text: "Folder"; note: "a git repository, so teams can work on their own branch" }
            Field { id: path; placeholderText: "C:\\path\\to\\project"; mono: true }
            Label { text: "Main thread's team" }
            Flow {
                Layout.fillWidth: true
                spacing: 6
                Repeater {
                    model: form.teams
                    delegate: Rectangle {
                        required property var modelData
                        readonly property bool on: form.team === modelData.name
                        implicitWidth: teamRow.implicitWidth + 20
                        implicitHeight: 32
                        radius: theme.radiusSmall
                        color: on ? theme.selection : (teamArea.containsMouse ? theme.field : "transparent")
                        border.color: on ? theme.blue : theme.line
                        Row {
                            id: teamRow
                            anchors.centerIn: parent
                            spacing: 10
                            TeamBadge { anchors.verticalCenter: parent.verticalCenter; team: modelData; size: 16 }
                            Text { anchors.verticalCenter: parent.verticalCenter; text: theme.who(modelData); color: theme.text; font.pixelSize: theme.small }
                        }
                        MouseArea { id: teamArea; anchors.fill: parent; hoverEnabled: true; onClicked: form.team = modelData.name }
                    }
                }
            }
            Label { text: "Permissions" }
            Rectangle {
                implicitWidth: levels.implicitWidth + 4
                implicitHeight: 30
                radius: 7
                color: theme.field
                Row {
                    id: levels
                    anchors.centerIn: parent
                    spacing: 2
                    Repeater {
                        model: form.agy ? ["full"] : ["plan", "ask", "sandbox", "full"]
                        delegate: Btn {
                            required property string modelData
                            kind: "quiet"; small: true
                            chosen: (form.agy ? "full" : form.freedom) === modelData
                            width: implicitWidth + 12
                            text: theme.freedomLabel(modelData)
                            onClicked: form.freedom = modelData
                        }
                    }
                }
            }
            RowLayout {
                visible: form.agy
                spacing: 6
                Icon { name: "warning"; size: 13; color: theme.amber }
                Text { text: "Antigravity can't ask for approval, so it always runs in Full auto."; color: theme.amber; font.pixelSize: theme.small }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: theme.pad
            Layout.rightMargin: theme.pad
            Layout.bottomMargin: theme.pad
            spacing: 6
            Item { Layout.fillWidth: true }
            Btn { kind: "quiet"; text: "Cancel"; onClicked: form.close() }
            Btn {
                kind: "primary"
                text: "Create"
                enabled: form.ready
                onClicked: form.create({ title: title.text.trim(), team: form.team, cwd: path.text.trim(), model: null, effort: null,
                                         permission_mode: form.agy ? "full" : form.freedom })
            }
        }
    }

    component Label: RowLayout {
        id: label
        property string text
        property string note
        Layout.topMargin: 8
        spacing: 8
        Text { text: label.text; color: theme.text2; font.pixelSize: theme.small }
        Text { visible: label.note !== ""; text: label.note; color: theme.text3; font.pixelSize: theme.tiny }
    }
    component Field: TextField {
        property bool mono: false
        Layout.fillWidth: true
        implicitHeight: 34
        leftPadding: 10
        color: theme.text
        placeholderTextColor: theme.text3
        font.family: mono ? form.monoFamily : Qt.application.font.family
        font.pixelSize: theme.size
        background: Rectangle { radius: theme.radiusSmall; color: theme.field; border.color: parent.activeFocus ? "#4A4A4A" : theme.line }
    }
}
