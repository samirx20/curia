import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// Settings > Freedom and approvals (docs: Design 17): the level new runs start with, the
// always-allow and never-allow lists, and what happens when nobody answers.
ColumnLayout {
    id: freedom
    property var page
    readonly property var s: page ? page.win.settings : ({})
    spacing: 4

    function add(key, text) {
        const v = text.trim()
        if (v === "") return
        page.setSetting(key, (s[key] || []).filter(x => x !== v).concat([v]))
    }
    function drop(key, v) { page.setSetting(key, (s[key] || []).filter(x => x !== v)) }

    SettingCard {
        title: "Default for new runs"
        note: "each run can change it"
        SettingRow {
            first: true
            title: "Freedom"
            hint: ({ plan: "Reads and plans, changes nothing.", ask: "Every risky action asks.", sandbox: "Edits inside the run's folder without asking.", full: "Only the never-allow list asks." })[freedom.s.default_freedom || "ask"]
            Segmented {
                options: [["plan", "Plan only"], ["ask", "Ask me"], ["sandbox", "Sandbox free"], ["full", "Full auto"]]
                value: freedom.s.default_freedom || "ask"
                onPicked: v => freedom.page.setSetting("default_freedom", v)
            }
        }
    }

    SettingCard {
        title: "Lists"
        note: "a tool name (Edit), or a command start (Bash:git status)"
        Repeater {
            model: [["always_allow", "Always allow", "Answered yes without asking."], ["never_allow", "Never allow", "Refused even on Full auto."]]
            delegate: Item {
                id: list
                required property var modelData
                required property int index
                Layout.fillWidth: true
                implicitHeight: col.implicitHeight + 28
                Rectangle { visible: list.index > 0; width: parent.width; height: 1; color: theme.line }
                ColumnLayout {
                    id: col
                    x: 16; y: 14
                    width: parent.width - 32
                    spacing: 8
                    Text { text: list.modelData[1]; color: theme.text; font.weight: Font.Medium }
                    Text { text: list.modelData[2]; color: theme.text3; font.pixelSize: theme.small }
                    Flow {
                        Layout.fillWidth: true
                        spacing: 6
                        Repeater {
                            model: freedom.s[list.modelData[0]] || []
                            delegate: Rectangle {
                                required property string modelData
                                implicitWidth: chipRow.implicitWidth + 16; implicitHeight: 26
                                radius: theme.radiusSmall; color: theme.selection
                                Row {
                                    id: chipRow
                                    anchors.centerIn: parent
                                    spacing: 6
                                    Text { anchors.verticalCenter: parent.verticalCenter; text: modelData; color: theme.text; font.family: "JetBrains Mono"; font.pixelSize: theme.small }
                                    Icon { anchors.verticalCenter: parent.verticalCenter; name: "close"; size: 11; color: theme.text3
                                        MouseArea { anchors.fill: parent; anchors.margins: -4; onClicked: freedom.drop(list.modelData[0], modelData) } }
                                }
                            }
                        }
                        TextField {
                            width: 180; implicitHeight: 26
                            placeholderText: "+ Add"
                            placeholderTextColor: theme.text3
                            color: theme.text
                            font.pixelSize: theme.small
                            onAccepted: { freedom.add(list.modelData[0], text); text = "" }
                            background: Rectangle { radius: theme.radiusSmall; color: "transparent"; border.color: theme.line }
                        }
                    }
                }
            }
        }
    }

    SettingCard {
        title: "When nobody answers"
        SettingRow {
            first: true
            title: "Deny after"
            hint: "Other threads keep working either way."
            Segmented {
                options: [[10, "10 min"], [30, "30 min"], [120, "2 h"], [null, "Never"]]
                value: freedom.s.deny_unanswered_after_minutes === undefined ? null : freedom.s.deny_unanswered_after_minutes
                onPicked: v => freedom.page.setSetting("deny_unanswered_after_minutes", v)
            }
        }
    }
}
