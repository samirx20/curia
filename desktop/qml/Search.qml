import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// Search (Ctrl K): threads by title, or by something said in them. Pick one to open it.
Rectangle {
    id: search
    property var results: []
    property var threads: []
    property var teams: []
    signal query(string text)
    signal pick(string threadId)
    signal close()

    color: theme.raised
    radius: theme.radius
    border.color: theme.line
    implicitWidth: 620
    implicitHeight: col.implicitHeight + 16
    function focusField() { field.text = ""; field.forceActiveFocus() }

    Timer { id: soon; interval: 200; onTriggered: search.query(field.text) }

    ColumnLayout {
        id: col
        x: 8; y: 8
        width: parent.width - 16
        spacing: 4
        RowLayout {
            Layout.fillWidth: true
            Layout.margins: 6
            spacing: 10
            Icon { name: "search"; size: 18; color: theme.text3 }
            TextField {
                id: field
                Layout.fillWidth: true
                placeholderText: "Search threads and what was said in them"
                placeholderTextColor: theme.text3
                color: theme.text
                font.pixelSize: 15
                background: null
                onTextChanged: soon.restart()
                Keys.onEscapePressed: search.close()
                Keys.onReturnPressed: if (search.results.length) search.pick(search.results[0].thread_id)
            }
            Text { text: "Esc"; color: theme.text3; font.pixelSize: theme.tiny }
        }
        Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: theme.line; visible: search.results.length > 0 || field.text !== "" }
        Text {
            visible: field.text.trim() !== "" && search.results.length === 0
            Layout.margins: 12
            text: "Nothing found."
            color: theme.text3
        }
        ListView {
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(contentHeight, 420)
            clip: true
            model: search.results
            delegate: Rectangle {
                required property var modelData
                readonly property var t: search.threads.find(x => x.id === modelData.thread_id)
                width: ListView.view.width
                height: hit.implicitHeight + 16
                radius: theme.radiusSmall
                color: area.containsMouse ? theme.selection : "transparent"
                RowLayout {
                    id: hit
                    x: 10; y: 8
                    width: parent.width - 20
                    spacing: 10
                    TeamBadge { team: t ? (search.teams.find(x => x.id === t.team_id) || { cli: t.cli, name: "" }) : null; Layout.alignment: Qt.AlignTop; Layout.rightMargin: 4 }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        Text { text: modelData.title; color: theme.text; font.weight: Font.DemiBold; elide: Text.ElideRight; Layout.fillWidth: true }
                        Text { visible: !!modelData.snippet; text: modelData.snippet || ""; color: theme.text2; font.pixelSize: theme.small; wrapMode: Text.Wrap; maximumLineCount: 2; elide: Text.ElideRight; Layout.fillWidth: true }
                    }
                }
                MouseArea { id: area; anchors.fill: parent; hoverEnabled: true; onClicked: search.pick(modelData.thread_id) }
            }
        }
    }
}
