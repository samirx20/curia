import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// The session rail (docs: Design 01, 05): the thread's subagents, facts, sandbox and its
// team's limits, or its files. The header's buttons pick the view.
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
    readonly property var run: thread ? threads.filter(t => t.root_id === thread.root_id && t.id !== thread.id) : []

    color: theme.panel
    radius: theme.radius

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // The view's name.
        RowLayout {
            Layout.fillWidth: true
            Layout.preferredHeight: 46
            Layout.leftMargin: theme.pad
            Layout.rightMargin: theme.pad
            Icon { name: rail.tab === "files" ? "folder" : "session"; size: 15; color: theme.text3 }
            Text { text: rail.tab === "files" ? "Files" : "Session"; color: theme.text; font.weight: Font.DemiBold; Layout.fillWidth: true; leftPadding: 4 }
        }
        Divider {}

        Files {
            visible: rail.tab === "files"
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.margins: theme.pad
            listing: rail.listing
            opened: rail.opened
            mono: rail.mono
            onOpenFile: path => rail.openFile(path)
            onBack: rail.closeFile()
        }

        ScrollView {
            visible: rail.tab === "session"
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
            ColumnLayout {
                width: rail.width
                spacing: 0

                Text {
                    visible: !rail.thread
                    Layout.margins: theme.pad
                    text: "No thread open."
                    color: theme.text3
                }

                // The thread and its team.
                Block {
                    visible: rail.thread !== null
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8
                        Text {
                            Layout.fillWidth: true
                            text: rail.thread ? rail.thread.title : ""
                            color: theme.text; font.weight: Font.DemiBold; elide: Text.ElideRight
                        }
                        Rectangle {
                            implicitWidth: teamChip.implicitWidth + 14
                            implicitHeight: 26
                            radius: theme.radiusSmall
                            color: theme.raised
                            Row {
                                id: teamChip
                                anchors.centerIn: parent
                                spacing: 9
                                TeamBadge { anchors.verticalCenter: parent.verticalCenter; team: rail.team; size: 16 }
                                Text { anchors.verticalCenter: parent.verticalCenter; text: theme.who(rail.team); color: theme.text2; font.pixelSize: theme.small }
                            }
                        }
                    }
                }

                // The run's other threads, when this is its main thread; otherwise subagents.
                Block {
                    visible: rail.thread !== null && (rail.subagents.length > 0 || (!rail.thread.parent_id && rail.run.length > 0))
                    Heading {
                        title: rail.subagents.length > 0 ? "Subagents" : "Team threads"
                        note: rail.subagents.length > 0 ? rail.subagents.filter(s => !s.done).length + " running"
                                                        : rail.run.filter(t => t.status === "running").length + " running"
                    }
                    Repeater {
                        model: rail.subagents.length > 0 ? rail.subagents : []
                        delegate: RowLayout {
                            required property var modelData
                            Layout.fillWidth: true
                            spacing: 8
                            StatusMark { size: 13; state_: modelData.done ? "done" : "running" }
                            Text { Layout.fillWidth: true; text: modelData.name; color: modelData.done ? theme.text3 : theme.text; elide: Text.ElideRight }
                        }
                    }
                    Repeater {
                        model: rail.subagents.length > 0 ? [] : rail.run
                        delegate: RowLayout {
                            required property var modelData
                            Layout.fillWidth: true
                            spacing: 8
                            StatusMark { size: 13; state_: modelData.status === "running" ? "running" : "done" }
                            Text { Layout.fillWidth: true; text: modelData.title; color: theme.text; elide: Text.ElideRight }
                            TeamBadge { team: rail.teams.find(t => t.id === modelData.team_id) || null; Layout.rightMargin: 4 }
                        }
                    }
                }

                Block {
                    visible: rail.thread !== null
                    Heading { title: "Thread" }
                    Fact { name: "Model"; value: rail.thread ? (rail.thread.model || "the CLI's default") : "" }
                    Fact { name: "Permissions"; value: rail.thread ? theme.freedomLabel(rail.thread.freedom) : ""; tint: rail.thread && rail.thread.freedom === "full" ? theme.amber : theme.text }
                    Fact { name: "Status"; value: rail.thread ? (rail.thread.status === "running" ? "Working" : "Idle") : "" }
                    Fact { name: "Updated"; value: rail.thread ? theme.ago(rail.thread.updated_at) + " ago" : "" }
                }

                Block {
                    visible: rail.thread !== null
                    Heading { title: "Sandbox" }
                    Fact { name: "Folder"; value: rail.thread ? rail.thread.cwd : ""; mono: true }
                    Fact { name: "Branch"; value: rail.thread ? (rail.thread.branch || "the folder's own") : ""; mono: rail.thread !== null && !!rail.thread.branch }
                    Btn {
                        Layout.topMargin: 4
                        Layout.fillWidth: true
                        icon: "folder"
                        text: "Changed files"
                        onClicked: { rail.tab = "files"; rail.listFiles() }
                    }
                }

                Block {
                    visible: rail.team !== null
                    divider: false
                    Heading { title: "Limits"; note: rail.team ? theme.who(rail.team) : "" }
                    Meter { label: "5-hour"; remaining: rail.team ? rail.team.five_hour_percent_left : null; resets: rail.team ? rail.team.five_hour_resets_at_ms : null }
                    Meter { label: "Week"; remaining: rail.team ? rail.team.weekly_percent_left : null; resets: rail.team ? rail.team.weekly_resets_at_ms : null }
                }
            }
        }
    }

    component Divider: Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: theme.line }
    // One section of the rail, with a divider under it.
    component Block: ColumnLayout {
        id: block
        default property alias content: blockBody.data
        property bool divider: true
        Layout.fillWidth: true
        spacing: 0
        ColumnLayout {
            id: blockBody
            Layout.fillWidth: true
            Layout.margins: theme.pad
            spacing: 9
        }
        Rectangle { visible: block.divider; Layout.fillWidth: true; Layout.preferredHeight: 1; color: theme.line }
    }
    component Heading: RowLayout {
        id: heading
        property string title
        property string note
        Layout.fillWidth: true
        Text { text: heading.title; color: theme.text3; font.pixelSize: theme.small; Layout.fillWidth: true }
        Text { text: heading.note; color: theme.text3; font.pixelSize: theme.small }
    }
    component Fact: RowLayout {
        id: fact
        property string name
        property string value
        property color tint: theme.text
        property bool mono: false
        Layout.fillWidth: true
        spacing: 12
        Text { text: fact.name; color: theme.text2 }
        Text {
            text: fact.value; color: fact.tint; elide: Text.ElideLeft; horizontalAlignment: Text.AlignRight; Layout.fillWidth: true
            font.family: fact.mono ? rail.mono : Qt.application.font.family
            font.pixelSize: fact.mono ? theme.small : theme.size
        }
    }
    component Meter: ColumnLayout {
        id: meter
        property string label
        property var remaining: null
        property var resets: null
        readonly property bool known: remaining !== null && remaining !== undefined
        Layout.fillWidth: true
        spacing: 6
        RowLayout {
            Layout.fillWidth: true
            Text { text: meter.label; color: theme.text; Layout.fillWidth: true }
            Text {
                color: theme.text3; font.pixelSize: theme.small
                text: !meter.known ? "not reported" : Math.round(meter.remaining) + "% left" + (meter.resets ? " · resets " + new Date(meter.resets).toLocaleString(Qt.locale(), "ddd HH:mm") : "")
            }
        }
        Rectangle {
            Layout.fillWidth: true; height: 3; radius: 2; color: theme.selection
            Rectangle {
                width: parent.width * (!meter.known ? 0 : (100 - meter.remaining) / 100); height: 3; radius: 2
                color: meter.known && meter.remaining < 15 ? theme.amber : theme.text2
            }
        }
    }
}
