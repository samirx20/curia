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
    implicitHeight: col.implicitHeight

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
            Icon { name: "usage"; size: 16; color: theme.text2 }
            Text { text: "Usage"; color: theme.text; font.weight: Font.DemiBold; Layout.fillWidth: true }
            Btn { kind: "icon"; icon: "close"; onClicked: panel.close() }
        }
        Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: theme.line }

        Repeater {
            model: panel.teams
            delegate: ColumnLayout {
                id: teamRow
                required property var modelData
                required property int index
                readonly property var t: modelData
                Layout.fillWidth: true
                spacing: 0
                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.margins: theme.pad
                    spacing: 8
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8
                        TeamBadge { team: teamRow.t; Layout.rightMargin: 4 }
                        Text { text: teamRow.t.name; color: theme.text; font.weight: Font.DemiBold }
                        Text { text: theme.cli(teamRow.t.cli).label + (teamRow.t.model ? " · " + teamRow.t.model : ""); color: theme.text3; font.pixelSize: theme.small; Layout.fillWidth: true; elide: Text.ElideRight }
                        Row {
                            visible: !!teamRow.t.out_of_quota_until_ms
                            spacing: 5
                            StatusMark { anchors.verticalCenter: parent.verticalCenter; size: 11; state_: "needs" }
                            Text { anchors.verticalCenter: parent.verticalCenter; text: "Out of quota"; color: theme.amber; font.pixelSize: theme.tiny }
                        }
                    }
                    Meter { label: "5-hour"; remaining: teamRow.t.five_hour_percent_left; resets: teamRow.t.five_hour_resets_at_ms }
                    Meter { label: "Week"; remaining: teamRow.t.weekly_percent_left; resets: teamRow.t.weekly_resets_at_ms }
                }
                Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: theme.line }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.margins: theme.pad
            spacing: 12
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 3
                Text { text: "Move work when a team runs out"; color: theme.text }
                Text { text: "to a team of the same CLI that can carry on the conversation"; color: theme.text3; font.pixelSize: theme.small; wrapMode: Text.Wrap; Layout.fillWidth: true }
            }
            Switch {
                id: reroute
                checked: panel.settings.reroute_when_out === true
                onToggled: panel.setSetting("reroute_when_out", checked)
                indicator: Rectangle {
                    implicitWidth: 36; implicitHeight: 20; radius: 10
                    color: reroute.checked ? theme.blue : theme.selection
                    Rectangle { x: reroute.checked ? parent.width - width - 3 : 3; y: 3; width: 14; height: 14; radius: 7; color: theme.text; Behavior on x { NumberAnimation { duration: 120 } } }
                }
            }
        }
    }

    component Meter: ColumnLayout {
        id: meter
        property string label
        property var remaining: null
        property var resets: null
        readonly property bool known: remaining !== null && remaining !== undefined
        Layout.fillWidth: true
        spacing: 5
        RowLayout {
            Layout.fillWidth: true
            Text { text: meter.label; color: theme.text2; font.pixelSize: theme.small; Layout.fillWidth: true }
            Text {
                color: theme.text3; font.pixelSize: theme.small
                text: !meter.known ? "not reported by this CLI" : Math.round(meter.remaining) + "% left" + (meter.resets ? " · resets " + new Date(meter.resets).toLocaleString(Qt.locale(), "ddd HH:mm") : "")
            }
        }
        Rectangle {
            Layout.fillWidth: true; height: 3; radius: 2; color: theme.selection
            Rectangle {
                width: parent.width * (!meter.known ? 0 : (100 - meter.remaining) / 100)
                height: 3; radius: 2
                color: meter.known && meter.remaining < 15 ? theme.amber : theme.text2
            }
        }
    }
}
