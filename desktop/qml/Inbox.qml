import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// Everything that needs you, across all runs, as a sheet on the right (docs: Design 03).
Rectangle {
    id: sheet
    property var items: []
    property var threads: []
    signal answer(string threadId, string requestId, var decision)
    signal openThread(string id)
    signal close()

    color: theme.panel
    radius: theme.radius
    border.color: theme.line

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 14
        spacing: 10
        RowLayout {
            Layout.fillWidth: true
            Text { text: "Inbox"; color: theme.text; font.weight: Font.DemiBold; Layout.fillWidth: true }
            Text { text: sheet.items.length + " need you"; color: sheet.items.length ? theme.amber : theme.text3 }
            Button { text: "Close"; flat: true; onClicked: sheet.close() }
        }
        Text { visible: sheet.items.length === 0; text: "Nothing needs you."; color: theme.text3 }
        ListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 8
            model: sheet.items
            delegate: Rectangle {
                required property var modelData
                readonly property var q: modelData
                width: ListView.view.width
                height: col.implicitHeight + 24
                radius: theme.radius
                color: theme.raised
                border.color: theme.amber
                ColumnLayout {
                    id: col
                    x: 12; y: 12
                    width: parent.width - 24
                    spacing: 6
                    Text { text: (q.kind === "question" ? "Asks you · " : "Wants to run · ") + q.thread_title; color: theme.text2; Layout.fillWidth: true; elide: Text.ElideRight }
                    Text {
                        Layout.fillWidth: true
                        wrapMode: Text.WrapAnywhere
                        color: theme.text
                        text: q.tool_name + "  " + (q.input.command || q.input.file_path || (q.input.questions ? q.input.questions[0].question : JSON.stringify(q.input)))
                    }
                    Row {
                        spacing: 8
                        Button { visible: q.kind !== "question"; text: "Allow"; onClicked: sheet.answer(q.thread_id, q.request_id, { decision: "allow" }) }
                        Button { visible: q.kind !== "question"; text: "Deny"; onClicked: sheet.answer(q.thread_id, q.request_id, { decision: "deny", message: null }) }
                        Button { text: "Open"; flat: true; onClicked: sheet.openThread(q.thread_id) }
                    }
                }
            }
        }
    }
}
