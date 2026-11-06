import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// The model picker (docs: Design 25): Team, Model, Thinking, Permissions. Picking another team
// moves the thread to it; the rest change how its CLI is started next.
Rectangle {
    id: picker
    property var thread: null
    property var teams: []
    signal moveTo(string team)
    signal configure(var options)
    signal close()

    readonly property var models: ["haiku", "sonnet", "opus"]
    readonly property var efforts: ["low", "medium", "high", "xhigh"]
    readonly property var levels: [
        { id: "plan", label: "Plan only", hint: "reads and plans, changes nothing" },
        { id: "ask", label: "Ask me", hint: "every risky action asks" },
        { id: "sandbox", label: "Sandbox free", hint: "edits in the sandbox without asking" },
        { id: "full", label: "Full auto", hint: "only the never-allow list asks" }
    ]

    color: theme.raised
    radius: theme.radius
    border.color: theme.line
    implicitHeight: col.implicitHeight + 24
    implicitWidth: 380

    ColumnLayout {
        id: col
        x: 12; y: 12
        width: parent.width - 24
        spacing: 4
        Heading { text: "Team" }
        Repeater {
            model: picker.teams
            delegate: Choice {
                required property var modelData
                label: modelData.name
                hint: modelData.five_hour_percent_left === null || modelData.five_hour_percent_left === undefined ? modelData.cli : Math.round(modelData.five_hour_percent_left) + "% of 5-hour left"
                chosen: picker.thread && picker.thread.team_id === modelData.id
                onPicked: picker.moveTo(modelData.name)
            }
        }
        Heading { text: "Model" }
        Flow {
            Layout.fillWidth: true
            spacing: 6
            Repeater {
                model: picker.models
                delegate: Pill { required property string modelData; label: modelData; chosen: picker.thread && picker.thread.model === modelData; onPicked: picker.configure({ model: modelData }) }
            }
        }
        Heading { text: "Thinking" }
        Flow {
            Layout.fillWidth: true
            spacing: 6
            Repeater {
                model: picker.efforts
                delegate: Pill { required property string modelData; label: modelData; onPicked: picker.configure({ effort: modelData }) }
            }
        }
        Heading { text: "Permissions" }
        Repeater {
            model: picker.levels
            delegate: Choice {
                required property var modelData
                label: modelData.label
                hint: modelData.hint
                chosen: picker.thread && picker.thread.freedom === modelData.id
                onPicked: picker.configure({ freedom: modelData.id })
            }
        }
    }

    component Heading: Text { color: theme.text3; font.pixelSize: theme.small; Layout.topMargin: 8 }
    component Choice: Rectangle {
        property string label
        property string hint
        property bool chosen: false
        signal picked()
        Layout.fillWidth: true
        height: 30
        radius: 6
        color: area.containsMouse ? theme.selection : "transparent"
        Text { x: 8; anchors.verticalCenter: parent.verticalCenter; text: (chosen ? "✓  " : "    ") + label; color: chosen ? theme.blue : theme.text }
        Text { anchors.right: parent.right; anchors.rightMargin: 8; anchors.verticalCenter: parent.verticalCenter; width: parent.width * 0.5; horizontalAlignment: Text.AlignRight; elide: Text.ElideRight; text: hint; color: theme.text3; font.pixelSize: theme.small }
        MouseArea { id: area; anchors.fill: parent; hoverEnabled: true; onClicked: parent.picked() }
    }
    component Pill: Rectangle {
        property string label
        property bool chosen: false
        signal picked()
        width: t.implicitWidth + 18
        height: 24
        radius: 12
        color: chosen ? theme.selection : theme.field
        border.color: chosen ? theme.blue : "transparent"
        Text { id: t; anchors.centerIn: parent; text: label; color: theme.text; font.pixelSize: theme.small }
        MouseArea { anchors.fill: parent; onClicked: parent.picked() }
    }
}
