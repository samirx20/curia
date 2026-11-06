import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// The model picker (docs: Design 25, 26): Team, Model, Thinking, Permissions. The lists follow
// the thread's CLI. Picking another team moves the thread to it; the rest change how its CLI is
// started next.
Rectangle {
    id: picker
    property var thread: null
    property var teams: []
    signal moveTo(string team)
    signal configure(var options)
    signal close()

    readonly property string cli: thread ? thread.cli : ""
    // What each CLI offers: Claude's and Antigravity's efforts from their `--help` (2026-11-07),
    // Codex's the three every version takes. Codex picks its model from the account, so only
    // the one in use is shown.
    readonly property var models: ({
        claude: ["haiku", "sonnet", "opus"],
        agy: ["gemini-3.8-flash-low", "gemini-3.8-flash-medium", "gemini-3.8-flash-high"]
    })[cli] || (thread && thread.model ? [thread.model] : [])
    readonly property var efforts: ({
        claude: ["low", "medium", "high", "xhigh", "max"],
        codex: ["low", "medium", "high"],
        agy: ["low", "medium", "high", "xhigh", "max"]
    })[cli] || []
    readonly property var levels: cli === "agy"
        ? [{ id: "full", hint: "Antigravity can't ask for approval; its own branch is the only fence" }]
        : [{ id: "plan", hint: "reads and plans, changes nothing" },
           { id: "ask", hint: "every risky action asks" },
           { id: "sandbox", hint: "edits in the sandbox without asking" },
           { id: "full", hint: "only the never-allow list asks" }]

    color: theme.raised
    radius: theme.radius
    border.color: theme.line
    implicitHeight: col.implicitHeight + 20
    implicitWidth: 400

    ColumnLayout {
        id: col
        x: 10; y: 10
        width: parent.width - 20
        spacing: 2

        Heading { text: "Team" }
        Repeater {
            model: picker.teams
            delegate: Choice {
                required property var modelData
                readonly property bool sameCli: modelData.cli === picker.cli
                badgeTeam: modelData
                label: theme.who(modelData)
                hint: !sameCli ? "another CLI: not yet" : modelData.five_hour_percent_left === null || modelData.five_hour_percent_left === undefined ? "" : Math.round(modelData.five_hour_percent_left) + "% of 5-hour left"
                chosen: picker.thread !== null && picker.thread.team_id === modelData.id
                enabled: sameCli
                onPicked: picker.moveTo(modelData.name)
            }
        }
        Divider {}
        Heading { text: "Model" }
        Flow {
            Layout.fillWidth: true
            Layout.leftMargin: 6
            Layout.bottomMargin: 4
            spacing: 6
            Repeater {
                model: picker.models
                delegate: Btn {
                    required property string modelData
                    kind: "secondary"; small: true
                    text: modelData
                    chosen: picker.thread !== null && picker.thread.model === modelData
                    border.width: chosen ? 1 : 0
                    border.color: theme.blue
                    onClicked: picker.configure({ model: modelData })
                }
            }
        }
        Heading { text: "Thinking" }
        Flow {
            Layout.fillWidth: true
            Layout.leftMargin: 6
            Layout.bottomMargin: 4
            spacing: 6
            Repeater {
                model: picker.efforts
                delegate: Btn { required property string modelData; kind: "secondary"; small: true; text: modelData; onClicked: picker.configure({ effort: modelData }) }
            }
        }
        Divider {}
        Heading { text: "Permissions" }
        Repeater {
            model: picker.levels
            delegate: Choice {
                required property var modelData
                label: theme.freedomLabel(modelData.id)
                hint: modelData.hint
                warn: picker.cli === "agy"
                chosen: picker.thread !== null && picker.thread.freedom === modelData.id
                onPicked: picker.configure({ freedom: modelData.id })
            }
        }
    }

    component Heading: Text { color: theme.text3; font.pixelSize: theme.small; Layout.topMargin: 8; Layout.bottomMargin: 4; Layout.leftMargin: 8 }
    component Divider: Rectangle { Layout.fillWidth: true; Layout.topMargin: 6; Layout.preferredHeight: 1; color: theme.line }
    component Choice: Rectangle {
        id: choice
        property string label
        property string hint
        property var badgeTeam: null
        property bool chosen: false
        property bool warn: false
        signal picked()
        Layout.fillWidth: true
        height: theme.row + 2
        radius: theme.radiusSmall
        opacity: enabled ? 1 : 0.45
        color: area.containsMouse && enabled ? theme.selection : "transparent"
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 8
            anchors.rightMargin: 8
            spacing: 8
            Item {
                implicitWidth: 14; implicitHeight: 14
                Icon { anchors.fill: parent; visible: choice.chosen; name: "check"; size: 14; stroke: 2.2; color: theme.blue }
            }
            TeamBadge { visible: choice.badgeTeam !== null; team: choice.badgeTeam; size: 16; Layout.rightMargin: 4 }
            Icon { visible: choice.warn; name: "warning"; size: 14; color: theme.amber }
            Text { text: choice.label; color: choice.chosen ? theme.text : theme.text2; font.weight: choice.chosen ? Font.DemiBold : Font.Normal }
            Text { Layout.fillWidth: true; horizontalAlignment: Text.AlignRight; elide: Text.ElideRight; text: choice.hint; color: choice.warn ? theme.amber : theme.text3; font.pixelSize: theme.tiny }
        }
        MouseArea { id: area; anchors.fill: parent; hoverEnabled: true; enabled: choice.enabled; onClicked: choice.picked() }
    }
}
