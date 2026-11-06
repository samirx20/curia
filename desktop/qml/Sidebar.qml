import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// Runs grouped by project folder, team threads nested under their main thread; the host and
// the app's own places at the bottom (docs: Design 01).
Rectangle {
    id: side
    property var threads: []
    property string currentId: ""
    property int inboxCount: 0
    property bool connected: false
    property string status: ""
    signal picked(string id)
    signal openInbox()
    signal openUsage()
    signal newRun()

    color: theme.panel
    radius: theme.radius

    // Folders in order of their newest run; each run with its team threads under it.
    readonly property var groups: {
        const runs = threads.filter(t => !t.parent_id)
        const byFolder = {}
        const order = []
        for (const r of runs) {
            const parts = r.cwd.split(/[\\/]/).filter(p => p !== "")
            const folder = parts.length ? parts[parts.length - 1] : r.cwd
            if (!byFolder[folder]) { byFolder[folder] = []; order.push(folder) }
            const rows = [{ thread: r, depth: 0 }]
            const under = id => threads.filter(t => t.parent_id === id)
            const walk = (id, depth) => { for (const c of under(id)) { rows.push({ thread: c, depth: depth }); walk(c.id, depth + 1) } }
            walk(r.id, 1)
            byFolder[folder] = byFolder[folder].concat(rows)
        }
        return order.map(f => ({ folder: f, rows: byFolder[f] }))
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 8
        spacing: 2

        SideItem { label: "New run"; glyph: "+"; hint: "Ctrl N"; onClicked: side.newRun() }
        SideItem { label: "Search"; glyph: "⌕"; hint: "Ctrl K" }
        SideItem {
            label: "Inbox"; glyph: "▭"
            badge: side.inboxCount
            onClicked: side.openInbox()
        }
        Item { Layout.preferredHeight: 10 }

        ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            ColumnLayout {
                width: side.width - 16
                spacing: 2
                Repeater {
                    model: side.groups
                    delegate: ColumnLayout {
                        required property var modelData
                        Layout.fillWidth: true
                        spacing: 2
                        Text {
                            text: "▭  " + modelData.folder + "  ⌄"
                            color: theme.text3
                            font.pixelSize: theme.small
                            font.weight: Font.DemiBold
                            Layout.topMargin: 10
                            Layout.leftMargin: 8
                        }
                        Repeater {
                            model: modelData.rows
                            delegate: Rectangle {
                                required property var modelData
                                readonly property var t: modelData.thread
                                Layout.fillWidth: true
                                height: 28
                                radius: 6
                                color: t.id === side.currentId ? theme.selection : (hover.containsMouse ? theme.raised : "transparent")
                                Text {
                                    id: mark
                                    x: 10 + modelData.depth * 16
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: t.status === "running" ? "◌" : "✓"
                                    color: t.status === "running" ? theme.blue : theme.text3
                                    font.pixelSize: theme.small
                                }
                                Text {
                                    anchors.left: mark.right
                                    anchors.leftMargin: 8
                                    anchors.right: parent.right
                                    anchors.rightMargin: 8
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: t.title
                                    elide: Text.ElideRight
                                    color: t.id === side.currentId ? theme.text : theme.text2
                                    font.pixelSize: theme.size
                                }
                                MouseArea { id: hover; anchors.fill: parent; hoverEnabled: true; onClicked: side.picked(t.id) }
                            }
                        }
                    }
                }
                Text {
                    visible: side.threads.length === 0
                    text: side.connected ? "No runs yet." : side.status
                    color: theme.text3
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                    Layout.margins: 10
                }
            }
        }

        Rectangle { Layout.fillWidth: true; height: 1; color: theme.line }
        RowLayout {
            Layout.fillWidth: true
            Layout.margins: 8
            Text { text: "▤  This computer"; color: theme.text; font.pixelSize: theme.size }
            Rectangle { width: 6; height: 6; radius: 3; color: side.connected ? theme.blue : theme.red }
            Text { text: side.connected ? "Local" : "Offline"; color: theme.text3; font.pixelSize: theme.small; Layout.fillWidth: true }
        }
        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 8
            Layout.bottomMargin: 6
            Text { text: "▥  Usage"; color: theme.text; Layout.fillWidth: true; MouseArea { anchors.fill: parent; onClicked: side.openUsage() } }
            Text { text: "✲  Settings"; color: theme.text; Layout.fillWidth: true }
        }
    }

    component SideItem: Rectangle {
        property string label
        property string glyph
        property string hint
        property int badge: 0
        signal clicked()
        Layout.fillWidth: true
        height: 30
        radius: 6
        color: area.containsMouse ? theme.raised : "transparent"
        Text { x: 10; anchors.verticalCenter: parent.verticalCenter; text: glyph + "   " + label; color: theme.text }
        Text { visible: hint !== ""; anchors.right: parent.right; anchors.rightMargin: 10; anchors.verticalCenter: parent.verticalCenter; text: hint; color: theme.text3; font.pixelSize: theme.small }
        Rectangle {
            visible: badge > 0
            anchors.right: parent.right; anchors.rightMargin: 10; anchors.verticalCenter: parent.verticalCenter
            width: 18; height: 18; radius: 9; color: theme.amber
            Text { anchors.centerIn: parent; text: badge; color: "#111"; font.pixelSize: 11; font.weight: Font.Bold }
        }
        MouseArea { id: area; anchors.fill: parent; hoverEnabled: true; onClicked: parent.clicked() }
    }
}
