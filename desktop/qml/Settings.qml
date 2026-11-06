import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// Settings (docs: Design 13–20): a full page. Sections under the host's name are saved on the
// host; "This computer" sections only here. Each section says which.
Rectangle {
    id: page
    property var win
    property string section: "host"
    property var info: ({})
    property var devices: []
    property var pairing: null
    property var settings: ({})
    property var teams: []
    signal close()

    color: theme.app
    readonly property var hostSections: [
        ["host", "host", "Host"], ["access", "approvals", "Access and devices"], ["teams", "people", "Teams"],
        ["routing", "handOut", "Hand-offs and routing"], ["freedom", "check", "Freedom and approvals"],
        ["notifications", "inbox", "Notifications"], ["data", "folder", "Data"], ["about", "message", "About"]
    ]
    readonly property var localSections: [["appearance", "settings", "Appearance"], ["shortcuts", "terminal", "Shortcuts"], ["hosts", "host", "Hosts"]]

    function send(tag, method, params) { win.send(tag, method, params) }
    function refresh() {
        send("s:info", "host_info")
        send("s:devices", "devices_list")
        send("teams", "teams_list")
        send("settings", "settings_get")
    }
    function replied(tag, value) {
        if (tag === "s:info") info = value
        else if (tag === "s:devices") devices = value
        else if (tag === "s:pair") pairing = value
        else if (tag === "s:changed") refresh()
    }
    // Saves one host setting, keeping the rest as they are.
    function setSetting(key, value) {
        const s = Object.assign({}, win.settings)
        s[key] = value
        send("settings", "settings_set", { settings: s })
    }
    onVisibleChanged: if (visible) refresh()

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: theme.gap
        spacing: theme.gap

        RowLayout {
            Layout.preferredHeight: 34
            spacing: 6
            Btn { kind: "icon"; icon: "back"; onClicked: page.close() }
            Text { text: "Settings"; color: theme.text; font.weight: Font.DemiBold }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: theme.gap

            // The sections.
            Rectangle {
                Layout.preferredWidth: 260
                Layout.fillHeight: true
                radius: theme.radius
                color: theme.panel
                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 8
                    spacing: 2
                    RowLayout {
                        Layout.fillWidth: true
                        Layout.margins: 8
                        Text { text: page.info.name || "This host"; color: theme.text2; font.weight: Font.DemiBold; font.pixelSize: theme.small; Layout.fillWidth: true; elide: Text.ElideRight }
                        Rectangle { width: 6; height: 6; radius: 3; color: theme.blue }
                        Text { text: "Host"; color: theme.text3; font.pixelSize: theme.tiny }
                    }
                    Repeater { model: page.hostSections; delegate: navItem }
                    Text { text: "This computer"; color: theme.text2; font.weight: Font.DemiBold; font.pixelSize: theme.small; Layout.margins: 8; Layout.topMargin: 16 }
                    Repeater { model: page.localSections; delegate: navItem }
                    Item { Layout.fillHeight: true }
                }
            }

            // The chosen section.
            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                radius: theme.radius
                color: theme.panel
                ScrollView {
                    id: scroll
                    anchors.fill: parent
                    clip: true
                    ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                    ColumnLayout {
                        width: Math.min(760, scroll.width - 80)
                        x: (scroll.width - width) / 2
                        spacing: 4
                        Item { Layout.preferredHeight: 24 }
                        Text {
                            text: (page.hostSections.concat(page.localSections).find(s => s[0] === page.section) || ["", "", ""])[2]
                            color: theme.text; font.pixelSize: 22; font.weight: Font.Bold
                        }
                        RowLayout {
                            spacing: 6
                            Icon { name: page.localSections.some(s => s[0] === page.section) ? "sidebar" : "host"; size: 13; color: theme.text3 }
                            Text {
                                text: page.localSections.some(s => s[0] === page.section) ? "Saved on this computer only." : "Saved on " + (page.info.name || "the host") + ". Applies to every device connected to it."
                                color: theme.text3; font.pixelSize: theme.small
                            }
                        }
                        Loader {
                            Layout.fillWidth: true
                            source: ({ host: "SettingsHost.qml", access: "SettingsAccess.qml", teams: "SettingsTeams.qml", routing: "SettingsRouting.qml",
                                       freedom: "SettingsFreedom.qml", hosts: "SettingsHosts.qml" })[page.section] || "SettingsOther.qml"
                            onLoaded: item.page = page
                        }
                        Item { Layout.preferredHeight: 40 }
                    }
                }
            }
        }
    }

    Component {
        id: navItem
        Rectangle {
            required property var modelData
            Layout.fillWidth: true
            height: theme.row
            radius: theme.radiusSmall
            color: page.section === modelData[0] ? theme.selection : (area.containsMouse ? theme.raised : "transparent")
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 10
                spacing: 10
                Icon { name: modelData[1]; size: 15; color: theme.text2 }
                Text { text: modelData[2]; color: theme.text; Layout.fillWidth: true }
                Rectangle { visible: modelData[0] === "teams" && page.win.teams.some(t => !t.signed_in); width: 6; height: 6; radius: 3; color: theme.amber; Layout.rightMargin: 10 }
            }
            MouseArea { id: area; anchors.fill: parent; hoverEnabled: true; onClicked: page.section = modelData[0] }
        }
    }
}
