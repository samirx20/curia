import QtQuick
import QtQuick.Layouts

// Settings' smaller sections: Notifications, Data, About, and This computer's Appearance and
// Shortcuts.
ColumnLayout {
    id: other
    property var page
    readonly property string part: page ? page.section : ""
    readonly property var info: page ? page.info : ({})
    spacing: 4

    SettingCard {
        visible: other.part === "notifications"
        title: "Phones and browsers"
        SettingRow {
            first: true
            title: "Approval notifications"
            hint: "Every paired device that may answer gets a notification when a request is still waiting after a couple of seconds, with Allow and Deny on Android. Turn them on in the phone app."
        }
        SettingRow {
            title: "What a notification says"
            hint: "Only the thread's name and which team asks. The rest loads when you open the app, so the push service can't read it."
        }
        SettingRow {
            title: "Internet needed"
            hint: "Notifications travel through Google's or Apple's push service, so the host needs internet to send them."
        }
    }

    SettingCard {
        visible: other.part === "data"
        title: "Where the host keeps things"
        SettingRow {
            first: true
            title: "Data folder"
            hint: other.info.data_dir || ""
            Btn { anchors.verticalCenter: parent.verticalCenter; text: "Open"; onClicked: Qt.openUrlExternally("file:///" + String(other.info.data_dir || "").replace(/\\/g, "/")) }
        }
        SettingRow {
            title: "What's in it"
            hint: "curia.sqlite (threads, events, inbox), transcripts (each CLI's raw output), worktrees (team threads' own checkouts), the host's keys."
        }
    }

    SettingCard {
        visible: other.part === "about"
        title: "Curia"
        SettingRow { first: true; title: "Version"; Text { anchors.verticalCenter: parent.verticalCenter; text: other.info.version || ""; color: theme.text } }
        SettingRow { title: "What it is"; hint: "A control room for agent CLIs: Claude Code, Codex and Antigravity, driven as they are, on your own accounts." }
        SettingRow { title: "Open-source parts"; hint: "Every library Curia uses and its licence is listed in ATTRIBUTIONS.md, which comes with the app." }
    }

    SettingCard {
        visible: other.part === "appearance"
        title: "Theme"
        SettingRow { first: true; title: "Dark"; hint: "Curia has one theme for now, made for long sessions." ; Icon { anchors.verticalCenter: parent.verticalCenter; name: "check"; size: 16; color: theme.blue } }
    }

    SettingCard {
        visible: other.part === "shortcuts"
        title: "Keys"
        Repeater {
            model: [["New run", "Ctrl N"], ["Search", "Ctrl K"], ["Send a message", "Enter"], ["New line in a message", "Shift Enter"], ["Close a panel", "Esc"]]
            delegate: SettingRow {
                required property var modelData
                required property int index
                first: index === 0
                title: modelData[0]
                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    implicitWidth: key.implicitWidth + 16; implicitHeight: 24; radius: 6; color: theme.selection
                    Text { id: key; anchors.centerIn: parent; text: modelData[1]; color: theme.text2; font.pixelSize: theme.small }
                }
            }
        }
    }
}
