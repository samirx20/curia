import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// Settings > Teams (docs: Design 15): each team is one signed-in CLI account. Edit a team's
// name and defaults, remove one no thread uses, or add one: pick the CLI and its account
// folder, sign it in on the host (the CLI's own sign-in, in a console window), then add it.
ColumnLayout {
    id: teams
    property var page
    property string editing: ""
    property bool adding: false
    property string newCli: "claude"
    readonly property var list: page ? page.win.teams : []
    readonly property var team: list.find(t => t.id === editing) || null
    readonly property var efforts: ({ claude: ["low", "medium", "high", "xhigh", "max"], codex: ["low", "medium", "high"], agy: ["low", "medium", "high", "xhigh", "max"] })
    spacing: 4

    function suggestFolder(cli) {
        const home = String(page.info.data_dir || "").replace(/[\\/][^\\/]*$/, "")
        return home + "\\accounts\\" + cli + "-" + (list.filter(t => t.cli === cli).length + 1)
    }

    SettingCard {
        title: "Accounts on this host"
        note: "each team is one signed-in CLI account"
        Repeater {
            model: teams.list
            delegate: SettingRow {
                required property var modelData
                required property int index
                first: index === 0
                team: modelData
                title: theme.who(modelData)
                hint: (modelData.signed_in ? "Signed in" : "Not signed in") + (modelData.five_hour_percent_left !== null && modelData.five_hour_percent_left !== undefined ? " · " + Math.round(100 - modelData.five_hour_percent_left) + "% of 5-hour limit used" : "") + " · " + modelData.config_dir
                Rectangle { anchors.verticalCenter: parent.verticalCenter; width: 6; height: 6; radius: 3; color: modelData.signed_in ? theme.blue : theme.amber }
                Btn { anchors.verticalCenter: parent.verticalCenter; visible: !modelData.signed_in; small: true; text: "Sign in"; onClicked: teams.page.send("s:changed", "team_sign_in", { cli: modelData.cli, config_dir: modelData.config_dir }) }
                Btn { anchors.verticalCenter: parent.verticalCenter; small: true; text: teams.editing === modelData.id ? "Done" : "Edit"; onClicked: { teams.adding = false; teams.editing = teams.editing === modelData.id ? "" : modelData.id } }
            }
        }
        Text { visible: teams.list.length === 0; Layout.margins: 16; text: "No teams yet. Add one to start a run."; color: theme.text3 }
    }
    Btn { Layout.alignment: Qt.AlignRight; icon: "plus"; text: "Add team"; visible: !teams.adding; onClicked: { teams.editing = ""; teams.adding = true; folder.text = teams.suggestFolder(teams.newCli) } }

    // Editing one team.
    SettingCard {
        visible: teams.team !== null
        title: teams.team ? theme.who(teams.team) : ""
        note: "editing"
        SettingRow {
            first: true
            title: "Name"
            hint: "Shown in threads and notifications."
            Field { text: teams.team ? teams.team.name : ""; onEditingFinished: teams.page.send("s:changed", "team_update", { id: teams.editing, name: text }) }
        }
        SettingRow {
            title: "Default model"
            hint: "Used for new threads on this team; a thread can still pick."
            Field { text: teams.team && teams.team.model ? teams.team.model : ""; placeholderText: "the CLI's default"; onEditingFinished: teams.page.send("s:changed", "team_update", { id: teams.editing, model: text.trim() }) }
        }
        SettingRow {
            title: "Effort"
            Segmented {
                options: teams.team ? (teams.efforts[teams.team.cli] || []).map(e => [e, e.charAt(0).toUpperCase() + e.slice(1)]) : []
                value: teams.team ? teams.team.effort : null
                onPicked: v => teams.page.send("s:changed", "team_update", { id: teams.editing, effort: v })
            }
        }
        SettingRow {
            title: "Remove this team"
            hint: "Only when no thread belongs to it. The account folder stays as it is."
            Btn { text: "Remove"; onClicked: { teams.page.send("s:changed", "team_remove", { id: teams.editing }); teams.editing = "" } }
        }
    }

    // Adding a team.
    SettingCard {
        visible: teams.adding
        title: "Add a team"
        SettingRow {
            first: true
            title: "CLI"
            Segmented {
                options: [["claude", "Claude Code"], ["codex", "Codex"], ["agy", "Antigravity"]]
                value: teams.newCli
                onPicked: v => { teams.newCli = v; folder.text = teams.suggestFolder(v) }
            }
        }
        SettingRow {
            title: "Name"
            hint: "Two words make the badge's two letters, e.g. \"work\" or \"personal\"."
            Field { id: name; placeholderText: "work" }
        }
        SettingRow {
            title: "Account folder"
            hint: "Where this CLI keeps this account's sign-in. A new folder means a new account."
            Field { id: folder; width: 300 }
        }
        SettingRow {
            title: "Sign in"
            hint: teams.newCli === "agy" ? "Antigravity signs in only in a console on this computer; one opens." : "The CLI's own sign-in opens in a console window on this computer."
            Btn { text: "Sign in on this computer"; enabled: folder.text.trim() !== ""; onClicked: teams.page.send("s:changed", "team_sign_in", { cli: teams.newCli, config_dir: folder.text.trim() }) }
        }
        SettingRow {
            title: ""
            Btn { kind: "quiet"; text: "Cancel"; onClicked: teams.adding = false }
            Btn {
                kind: "primary"; text: "Add team"
                enabled: name.text.trim() !== "" && folder.text.trim() !== ""
                onClicked: {
                    teams.page.send("s:changed", "team_create", { team: { name: name.text.trim(), cli: teams.newCli, config_dir: folder.text.trim(), model: null, effort: null } })
                    teams.adding = false
                    name.text = ""
                }
            }
        }
    }

    component Field: TextField {
        width: 240
        anchors.verticalCenter: parent ? parent.verticalCenter : undefined
        color: theme.text
        placeholderTextColor: theme.text3
        background: Rectangle { radius: theme.radiusSmall; color: theme.field; border.color: parent.activeFocus ? "#4a4a4a" : theme.line }
    }
}
