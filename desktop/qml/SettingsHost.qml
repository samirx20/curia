import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// Settings > Host (docs: Design 13): the machine, its resources, and how the host runs.
ColumnLayout {
    id: host
    property var page
    readonly property var info: page ? page.info : ({})
    spacing: 4

    function gb(b) { return (b / 1073741824).toFixed(1) }
    function since(ms) {
        const m = Math.round((Date.now() - ms) / 60000)
        return m < 60 ? m + " min" : m < 1440 ? Math.round(m / 60) + " h" : Math.round(m / 1440) + " days"
    }

    Rectangle {
        Layout.fillWidth: true
        Layout.topMargin: 14
        implicitHeight: 78
        radius: theme.radius
        color: theme.card
        border.color: theme.line
        RowLayout {
            anchors.fill: parent
            anchors.margins: 16
            spacing: 14
            Rectangle {
                width: 44; height: 44; radius: 10; color: theme.selection
                Icon { anchors.centerIn: parent; name: "host"; size: 22; color: theme.text2 }
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 3
                RowLayout {
                    spacing: 8
                    Text { text: host.info.name || "…"; color: theme.text; font.pixelSize: 16; font.weight: Font.DemiBold }
                    Rectangle { width: 6; height: 6; radius: 3; color: theme.blue }
                    Text { text: "Online"; color: theme.text2; font.pixelSize: theme.small }
                }
                Text {
                    Layout.fillWidth: true
                    elide: Text.ElideRight
                    color: theme.text3; font.pixelSize: theme.small
                    text: [host.info.os, "Curia host " + (host.info.version || ""), host.info.started_ms ? "up " + host.since(host.info.started_ms) : "",
                           host.info.remote_url ? "reached through Tailscale" : "this computer only"].filter(x => x).join(" · ")
                }
            }
        }
    }

    SettingCard {
        title: "Resources"
        note: "right now"
        SettingRow {
            first: true
            title: "Processor"
            Meter { value: (host.info.cpu_percent || 0) / 100 }
            Text { anchors.verticalCenter: parent.verticalCenter; text: Math.round(host.info.cpu_percent || 0) + "% of " + (host.info.cores || "?") + " cores"; color: theme.text; width: 130; horizontalAlignment: Text.AlignRight }
        }
        SettingRow {
            title: "Memory"
            hint: "Curia keeps about 3 GB free for you."
            Meter { value: host.info.memory ? host.info.memory.used / host.info.memory.total : 0 }
            Text { anchors.verticalCenter: parent.verticalCenter; text: host.info.memory ? host.gb(host.info.memory.used) + " of " + host.gb(host.info.memory.total) + " GB" : ""; color: theme.text; width: 130; horizontalAlignment: Text.AlignRight }
        }
        SettingRow {
            title: "Disk"
            hint: "Where the host keeps its data and sandboxes."
            Meter { value: host.info.disk ? host.info.disk.used / host.info.disk.total : 0; warn: !!host.info.disk && host.info.disk.used / host.info.disk.total > 0.8 }
            Text { anchors.verticalCenter: parent.verticalCenter; text: host.info.disk ? Math.round(host.gb(host.info.disk.used)) + " of " + Math.round(host.gb(host.info.disk.total)) + " GB" : ""; color: theme.text; width: 130; horizontalAlignment: Text.AlignRight }
        }
    }

    SettingCard {
        title: "Service"
        SettingRow {
            first: true
            title: "Host name"
            hint: "Shown on every device and in notifications."
            TextField {
                id: nameField
                width: 220
                text: host.page && host.page.win.settings.host_name ? host.page.win.settings.host_name : ""
                placeholderText: host.info.name || ""
                placeholderTextColor: theme.text3
                color: theme.text
                onEditingFinished: host.page.setSetting("host_name", text.trim() === "" ? null : text.trim())
                background: Rectangle { radius: theme.radiusSmall; color: theme.field; border.color: parent.activeFocus ? "#4a4a4a" : theme.line }
            }
        }
        SettingRow {
            title: "Start when you sign in"
            hint: "The host starts by itself, with no window, when you sign in to this computer, and again if it stops. Runs pick up where they were. Its log is host.log in the data folder."
            Toggle {
                on: !!host.info.autostart
                onToggled: on => host.page.send("s:changed", "autostart_set", { enabled: on })
            }
        }
        SettingRow {
            title: "Version"
            Text { anchors.verticalCenter: parent.verticalCenter; text: host.info.version || ""; color: theme.text }
        }
        SettingRow {
            title: "Data"
            hint: host.info.data_dir || ""
            Btn { text: "Open folder"; onClicked: Qt.openUrlExternally("file:///" + String(host.info.data_dir || "").replace(/\\/g, "/")) }
        }
    }

    component Meter: Rectangle {
        property real value: 0
        property bool warn: false
        anchors.verticalCenter: parent.verticalCenter
        width: 180; height: 4; radius: 2; color: theme.selection
        Rectangle { width: parent.width * Math.max(0, Math.min(1, parent.value)); height: 4; radius: 2; color: parent.warn ? theme.amber : theme.text2 }
    }
}
