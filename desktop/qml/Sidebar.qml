import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// Runs grouped by project folder, team threads nested under their main thread; the host and
// the app's own places at the bottom (docs: Design 01).
Rectangle {
    id: side
    property var threads: []
    property var teams: []
    property var inbox: []
    property string currentId: ""
    property bool connected: false
    property string status: ""
    signal picked(string id)
    signal openInbox()
    signal openUsage()
    signal newRun(string folder)

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
            if (!byFolder[folder]) { byFolder[folder] = { path: r.cwd, rows: [] }; order.push(folder) }
            const rows = [{ thread: r, depth: 0 }]
            const under = id => threads.filter(t => t.parent_id === id)
            const walk = (id, depth) => { for (const c of under(id)) { rows.push({ thread: c, depth: depth }); walk(c.id, depth + 1) } }
            walk(r.id, 1)
            byFolder[folder].rows = byFolder[folder].rows.concat(rows)
        }
        return order.map(f => ({ folder: f, path: byFolder[f].path, rows: byFolder[f].rows }))
    }
    property var collapsed: ({})
    readonly property bool usageLow: teams.some(t => t.five_hour_percent_left !== null && t.five_hour_percent_left !== undefined && t.five_hour_percent_left < 15)
    function needsYou(id) { return inbox.some(q => q.thread_id === id) }
    function teamOf(t) { return teams.find(x => x.id === t.team_id) || { cli: t.cli, name: "" } }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 8
        spacing: 2

        Place { label: "New run"; icon: "plus"; hint: "Ctrl N"; onClicked: side.newRun("") }
        Place { label: "Search"; icon: "search"; hint: "Ctrl K"; enabled: false }
        Place { label: "Inbox"; icon: "inbox"; badge: side.inbox.length; onClicked: side.openInbox() }
        Item { Layout.preferredHeight: 8 }

        ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
            ColumnLayout {
                width: side.width - 16
                spacing: 2
                Repeater {
                    model: side.groups
                    delegate: ColumnLayout {
                        id: group
                        required property var modelData
                        readonly property bool shut: side.collapsed[modelData.folder] === true
                        Layout.fillWidth: true
                        spacing: 2
                        // The folder's heading: click to fold; "+" starts a run in it.
                        Rectangle {
                            Layout.fillWidth: true
                            Layout.topMargin: 10
                            height: 26
                            radius: theme.radiusSmall
                            color: "transparent"
                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 8
                                anchors.rightMargin: 6
                                spacing: 6
                                Icon { name: "folder"; size: 14; color: theme.text3; stroke: 1.7 }
                                Text { text: group.modelData.folder; color: theme.text2; font.pixelSize: theme.small; font.weight: Font.DemiBold }
                                Icon { name: group.shut ? "open" : "down"; size: 12; color: theme.text3; stroke: 2 }
                                Item { Layout.fillWidth: true }
                                Btn { kind: "icon"; icon: "plus"; small: true; visible: head.containsMouse; onClicked: side.newRun(group.modelData.path) }
                            }
                            MouseArea {
                                id: head
                                anchors.fill: parent
                                anchors.rightMargin: 30
                                hoverEnabled: true
                                propagateComposedEvents: true
                                onClicked: { const c = Object.assign({}, side.collapsed); c[group.modelData.folder] = !group.shut; side.collapsed = c }
                            }
                        }
                        Repeater {
                            model: group.shut ? [] : group.modelData.rows
                            delegate: Rectangle {
                                id: rowItem
                                required property var modelData
                                readonly property var t: modelData.thread
                                readonly property bool current: t.id === side.currentId
                                Layout.fillWidth: true
                                height: 28
                                radius: theme.radiusSmall
                                color: current ? theme.selection : (hover.containsMouse ? theme.raised : "transparent")
                                // A thin line ties team threads to their main thread.
                                Rectangle {
                                    visible: rowItem.modelData.depth > 0
                                    x: 16 + (rowItem.modelData.depth - 1) * 16
                                    width: 1; height: parent.height
                                    color: theme.line
                                }
                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 10 + rowItem.modelData.depth * 16
                                    anchors.rightMargin: 10
                                    spacing: 8
                                    StatusMark {
                                        size: 13
                                        state_: side.needsYou(rowItem.t.id) ? "needs" : rowItem.t.status === "running" ? "running" : "done"
                                    }
                                    Text {
                                        Layout.fillWidth: true
                                        text: rowItem.t.title
                                        elide: Text.ElideRight
                                        color: rowItem.current ? theme.text : theme.text2
                                        font.weight: rowItem.current ? Font.DemiBold : Font.Normal
                                    }
                                    TeamBadge { team: side.teamOf(rowItem.t); Layout.rightMargin: 4 }
                                }
                                MouseArea { id: hover; anchors.fill: parent; hoverEnabled: true; onClicked: side.picked(rowItem.t.id) }
                            }
                        }
                    }
                }
                Text {
                    visible: side.threads.length === 0
                    text: side.connected ? "No runs yet. Start one with New run." : side.status
                    color: theme.text3
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                    Layout.margins: 10
                }
            }
        }

        Rectangle { Layout.fillWidth: true; Layout.bottomMargin: 4; height: 1; color: theme.line }
        Rectangle {
            Layout.fillWidth: true
            height: theme.row
            radius: theme.radiusSmall
            color: "transparent"
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 10
                anchors.rightMargin: 10
                spacing: 8
                Icon { name: "host"; size: 15; color: theme.text2; stroke: 1.7 }
                Text { text: "This computer"; color: theme.text; font.weight: Font.DemiBold }
                Rectangle { width: 6; height: 6; radius: 3; color: side.connected ? theme.blue : theme.red }
                Text { text: side.connected ? "Local" : "Offline"; color: theme.text3; font.pixelSize: theme.small; Layout.fillWidth: true }
                Icon { name: "upDown"; size: 13; color: theme.text3; stroke: 2 }
            }
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 2
            Place { label: "Usage"; icon: "usage"; dot: side.usageLow; Layout.fillWidth: true; Layout.preferredWidth: 1; onClicked: side.openUsage() }
            Place { label: "Settings"; icon: "settings"; enabled: false; Layout.fillWidth: true; Layout.preferredWidth: 1 }
        }
    }

    // One of the app's own places: icon, name, and a shortcut, count or dot on the right.
    component Place: Rectangle {
        id: place
        property string label
        property string icon
        property string hint
        property int badge: 0
        property bool dot: false
        signal clicked()
        Layout.fillWidth: true
        height: theme.row
        radius: theme.radiusSmall
        opacity: enabled ? 1 : 0.5
        color: area.containsMouse && enabled ? theme.raised : "transparent"
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 10
            anchors.rightMargin: 10
            spacing: 10
            Icon { name: place.icon; size: 15; color: theme.text2; stroke: 1.8 }
            Text { text: place.label; color: theme.text }
            Rectangle { visible: place.dot; width: 6; height: 6; radius: 3; color: theme.amber }
            Item { Layout.fillWidth: true }
            Text { visible: place.hint !== ""; text: place.hint; color: theme.text3; font.pixelSize: theme.tiny }
            Rectangle {
                visible: place.badge > 0
                width: Math.max(18, count.implicitWidth + 8); height: 18; radius: 9; color: theme.amber
                Text { id: count; anchors.centerIn: parent; text: place.badge; color: theme.onLight; font.pixelSize: theme.tiny; font.weight: Font.Bold }
            }
        }
        MouseArea { id: area; anchors.fill: parent; hoverEnabled: true; enabled: place.enabled; onClicked: place.clicked() }
        ToolTip.visible: !place.enabled && hoverAll.containsMouse
        ToolTip.text: "Not built yet"
        MouseArea { id: hoverAll; anchors.fill: parent; hoverEnabled: true; acceptedButtons: Qt.NoButton }
    }
}
