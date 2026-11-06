import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// The session rail (docs: Design 01): subagents, the thread's facts, its team's limits.
Rectangle {
    id: rail
    property var thread: null
    property var threads: []
    property var teams: []
    property var subagents: []
    property string tab: "session"
    property var listing: ({ git: true, files: [] })
    property var opened: null
    property string mono: ""
    signal listFiles()
    signal openFile(string path)
    signal closeFile()
    readonly property var team: thread ? teams.find(t => t.id === thread.team_id) || null : null

    color: theme.panel
    radius: theme.radius

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 14
        spacing: 10

        Row {
            spacing: 14
            Repeater {
                model: [["session", "Session"], ["files", "Files"]]
                delegate: Text {
                    required property var modelData
                    text: modelData[1]
                    color: rail.tab === modelData[0] ? theme.text : theme.text3
                    font.weight: rail.tab === modelData[0] ? Font.DemiBold : Font.Normal
                    MouseArea { anchors.fill: parent; onClicked: { rail.tab = modelData[0]; if (rail.tab === "files") rail.listFiles() } }
                }
            }
        }
        Files {
            visible: rail.tab === "files"
            Layout.fillWidth: true
            Layout.fillHeight: true
            listing: rail.listing
            opened: rail.opened
            mono: rail.mono
            onOpenFile: path => rail.openFile(path)
            onBack: rail.closeFile()
        }
        ColumnLayout {
        visible: rail.tab === "session"
        Layout.fillWidth: true
        Layout.fillHeight: true
        spacing: 10
        Text {
            visible: !rail.thread
            text: "No thread open."
            color: theme.text3
        }
        Text {
            visible: rail.thread !== null
            Layout.fillWidth: true
            text: rail.thread ? rail.thread.title : ""
            color: theme.text; font.weight: Font.DemiBold; elide: Text.ElideRight
        }

        Section { title: "Subagents"; note: rail.subagents.filter(s => !s.done).length + " running"; visible: rail.subagents.length > 0 }
        Repeater {
            model: rail.subagents
            delegate: Text {
                required property var modelData
                Layout.fillWidth: true
                text: (modelData.done ? "✓  " : "◌  ") + modelData.name
                color: modelData.done ? theme.text3 : theme.text
                elide: Text.ElideRight
            }
        }

        Section { title: "Thread"; visible: rail.thread !== null }
        Fact { name: "Team"; value: rail.team ? rail.team.name : "—" }
        Fact { name: "Model"; value: rail.thread ? (rail.thread.model || "default") : "" }
        Fact { name: "Freedom"; value: rail.thread ? rail.thread.freedom : "" }
        Fact { name: "Status"; value: rail.thread ? rail.thread.status : "" }

        Section { title: "Sandbox"; visible: rail.thread !== null }
        Fact { name: "Folder"; value: rail.thread ? rail.thread.cwd : "" }
        Fact { name: "Branch"; value: rail.thread ? (rail.thread.branch || "the folder's own") : "" }

        Section { title: "Limits"; note: rail.team ? rail.team.name : ""; visible: rail.team !== null }
        Bar { label: "5-hour"; remaining: rail.team ? rail.team.five_hour_percent_left : null; resets: rail.team ? rail.team.five_hour_resets_at_ms : null }
        Bar { label: "Week"; remaining: rail.team ? rail.team.weekly_percent_left : null; resets: rail.team ? rail.team.weekly_resets_at_ms : null }

        Item { Layout.fillHeight: true }
        }
    }

    component Section: RowLayout {
        property string title
        property string note
        Layout.fillWidth: true
        Layout.topMargin: 8
        Text { text: title; color: theme.text3; font.pixelSize: theme.small; Layout.fillWidth: true }
        Text { text: note; color: theme.text3; font.pixelSize: theme.small }
    }
    component Fact: RowLayout {
        property string name
        property string value
        visible: rail.thread !== null
        Layout.fillWidth: true
        Text { text: name; color: theme.text2 }
        Text { text: value; color: theme.text; elide: Text.ElideLeft; horizontalAlignment: Text.AlignRight; Layout.fillWidth: true }
    }
    component Bar: ColumnLayout {
        property string label
        property var remaining: null
        property var resets: null
        visible: rail.team !== null
        Layout.fillWidth: true
        spacing: 4
        RowLayout {
            Layout.fillWidth: true
            Text { text: label; color: theme.text2; Layout.fillWidth: true }
            Text {
                color: theme.text3; font.pixelSize: theme.small
                text: remaining === null ? "not reported" : Math.round(remaining) + "% left" + (resets ? " · resets " + new Date(resets).toLocaleString(Qt.locale(), "ddd HH:mm") : "")
            }
        }
        Rectangle {
            Layout.fillWidth: true; height: 4; radius: 2; color: theme.field
            Rectangle { width: parent.width * (remaining === null ? 0 : (100 - remaining) / 100); height: 4; radius: 2; color: remaining !== null && remaining < 15 ? theme.amber : theme.text2 }
        }
    }
}
