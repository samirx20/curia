import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// Teams with their limits, and the switch that reroutes work when a team runs out (docs: Design 04).
Rectangle {
    id: panel
    property var teams: []
    property var settings: ({})
    signal setSetting(string key, var value)
    signal close()

    color: theme.panel
    radius: theme.radius
    border.color: theme.line
    implicitHeight: col.implicitHeight + 28

    ColumnLayout {
        id: col
        x: 14; y: 14
        width: parent.width - 28
        spacing: 12
        RowLayout {
            Layout.fillWidth: true
            Text { text: "Usage"; color: theme.text; font.weight: Font.DemiBold; Layout.fillWidth: true }
            Button { text: "Close"; flat: true; onClicked: panel.close() }
        }
        Repeater {
            model: panel.teams
            delegate: ColumnLayout {
                required property var modelData
                readonly property var t: modelData
                Layout.fillWidth: true
                spacing: 4
                RowLayout {
                    Layout.fillWidth: true
                    Text { text: t.name; color: theme.text; Layout.fillWidth: true }
                    Text { text: t.cli + (t.model ? " · " + t.model : ""); color: theme.text3; font.pixelSize: theme.small }
                    Text { visible: !!t.out_of_quota_until_ms; text: "  out of quota"; color: theme.amber; font.pixelSize: theme.small }
                }
                Meter { label: "5-hour"; remaining: t.five_hour_percent_left; resets: t.five_hour_resets_at_ms }
                Meter { label: "Week"; remaining: t.weekly_percent_left; resets: t.weekly_resets_at_ms }
            }
        }
        Rectangle { Layout.fillWidth: true; height: 1; color: theme.line }
        RowLayout {
            Layout.fillWidth: true
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2
                Text { text: "Move work when a team runs out"; color: theme.text }
                Text { text: "to a team of the same CLI that can carry on the conversation"; color: theme.text3; font.pixelSize: theme.small; wrapMode: Text.Wrap; Layout.fillWidth: true }
            }
            Switch {
                id: reroute
                checked: panel.settings.reroute_when_out === true
                onToggled: panel.setSetting("reroute_when_out", checked)
                indicator: Rectangle {
                    implicitWidth: 36; implicitHeight: 20; radius: 10
                    color: reroute.checked ? theme.blue : theme.field
                    border.color: theme.line
                    Rectangle { x: reroute.checked ? parent.width - width - 3 : 3; y: 3; width: 14; height: 14; radius: 7; color: theme.text }
                }
            }
        }
    }

    component Meter: ColumnLayout {
        property string label
        property var remaining: null
        property var resets: null
        Layout.fillWidth: true
        spacing: 3
        RowLayout {
            Layout.fillWidth: true
            Text { text: label; color: theme.text2; font.pixelSize: theme.small; Layout.fillWidth: true }
            Text {
                color: theme.text3; font.pixelSize: theme.small
                text: remaining === null || remaining === undefined ? "not reported" : Math.round(remaining) + "% left" + (resets ? " · resets " + new Date(resets).toLocaleString(Qt.locale(), "ddd HH:mm") : "")
            }
        }
        Rectangle {
            Layout.fillWidth: true; height: 4; radius: 2; color: theme.field
            Rectangle {
                width: parent.width * (remaining === null || remaining === undefined ? 0 : (100 - remaining) / 100)
                height: 4; radius: 2
                color: remaining !== null && remaining !== undefined && remaining < 15 ? theme.amber : theme.text2
            }
        }
    }
}
