import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// Everything that needs you, across all runs, as a sheet on the right (docs: Design 03).
Rectangle {
    id: sheet
    property var items: []
    property var threads: []
    property var teams: []
    property string mono: ""
    signal answer(string threadId, string requestId, var decision)
    signal openThread(string id)
    signal close()

    function teamOf(threadId) {
        const t = threads.find(x => x.id === threadId)
        return t ? teams.find(x => x.id === t.team_id) || { cli: t.cli, name: "" } : null
    }

    color: theme.panel
    radius: theme.radius
    border.color: theme.line

    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        RowLayout {
            Layout.fillWidth: true
            Layout.preferredHeight: 50
            Layout.leftMargin: theme.pad
            Layout.rightMargin: 8
            spacing: 8
            Icon { name: "inbox"; size: 16; color: theme.text2 }
            Text { text: "Inbox"; color: theme.text; font.weight: Font.DemiBold }
            Rectangle {
                visible: sheet.items.length > 0
                width: Math.max(18, n.implicitWidth + 8); height: 18; radius: 9; color: theme.amber
                Text { id: n; anchors.centerIn: parent; text: sheet.items.length; color: theme.onLight; font.pixelSize: theme.tiny; font.weight: Font.Bold }
            }
            Item { Layout.fillWidth: true }
            Btn { kind: "icon"; icon: "close"; onClicked: sheet.close() }
        }
        Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: theme.line }

        ColumnLayout {
            visible: sheet.items.length === 0
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 8
            Item { Layout.fillHeight: true }
            Icon { Layout.alignment: Qt.AlignHCenter; name: "approvals"; size: 28; stroke: 1.5; color: theme.text3 }
            Text { Layout.fillWidth: true; horizontalAlignment: Text.AlignHCenter; text: "Nothing needs you."; color: theme.text2 }
            Text {
                Layout.fillWidth: true; Layout.leftMargin: 24; Layout.rightMargin: 24
                horizontalAlignment: Text.AlignHCenter; wrapMode: Text.Wrap
                text: "Approvals and questions from every run land here."; color: theme.text3; font.pixelSize: theme.small
            }
            Item { Layout.fillHeight: true }
        }
        ListView {
            visible: sheet.items.length > 0
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.margins: 10
            clip: true
            spacing: 10
            model: sheet.items
            delegate: Rectangle {
                id: card
                required property var modelData
                readonly property var q: modelData
                width: ListView.view.width
                height: col.implicitHeight + 24
                radius: theme.radius
                color: theme.card
                border.color: theme.amber
                ColumnLayout {
                    id: col
                    x: 14; y: 12
                    width: parent.width - 28
                    spacing: 8
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8
                        TeamBadge { team: sheet.teamOf(card.q.thread_id); size: 16; Layout.rightMargin: 4 }
                        Text { text: card.q.thread_title; color: theme.text; font.weight: Font.DemiBold; elide: Text.ElideRight; Layout.fillWidth: true }
                        StatusMark { size: 12; state_: "needs" }
                        Text { text: card.q.kind === "question" ? "Asks you" : "Wants to run"; color: theme.amber; font.pixelSize: theme.tiny }
                    }
                    Rectangle {
                        Layout.fillWidth: true
                        implicitHeight: well.implicitHeight + 20
                        radius: 8
                        color: theme.well
                        ColumnLayout {
                            id: well
                            x: 12; y: 10
                            width: parent.width - 24
                            spacing: 6
                            Text { text: card.q.tool_name.replace(/^mcp__([^_]+)__/, "$1: "); color: theme.text3; font.pixelSize: theme.tiny }
                            Text {
                                Layout.fillWidth: true
                                wrapMode: Text.WrapAnywhere
                                color: theme.text
                                font.family: sheet.mono
                                font.pixelSize: theme.small
                                text: card.q.input.command || card.q.input.file_path || (card.q.input.questions ? card.q.input.questions[0].question : JSON.stringify(card.q.input))
                            }
                        }
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 6
                        Btn { visible: card.q.kind !== "question"; kind: "primary"; text: "Allow"; onClicked: sheet.answer(card.q.thread_id, card.q.request_id, { decision: "allow" }) }
                        Btn { visible: card.q.kind !== "question"; text: "Deny"; onClicked: sheet.answer(card.q.thread_id, card.q.request_id, { decision: "deny", message: null }) }
                        Item { Layout.fillWidth: true }
                        Btn { kind: "quiet"; icon: "open"; text: "Open thread"; onClicked: sheet.openThread(card.q.thread_id) }
                    }
                }
            }
        }
    }
}
