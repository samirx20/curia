import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// Settings > Hosts (this computer): the hosts this app can follow, and connecting to a host on
// another machine. The app pairs with it the way a phone does, with its own key.
ColumnLayout {
    id: hostsPage
    property var page
    readonly property var win: page ? page.win : null
    readonly property var known: win ? (win.hostsVersion, JSON.parse(host.hostsJson())) : []
    spacing: 4

    SettingCard {
        title: "Hosts this app knows"
        Repeater {
            model: hostsPage.known
            delegate: SettingRow {
                required property var modelData
                required property int index
                first: index === 0
                title: modelData.name
                hint: (modelData.id === "local" ? "This computer" : modelData.where + " · " + modelData.role)
                      + (modelData.current ? (host.connected ? " · connected" : " · not reachable right now") : "")
                Rectangle { visible: modelData.current; anchors.verticalCenter: parent.verticalCenter; width: 6; height: 6; radius: 3; color: host.connected ? theme.blue : theme.red }
                Text { visible: modelData.current; anchors.verticalCenter: parent.verticalCenter; text: "Following"; color: theme.text2 }
                Btn { visible: !modelData.current; anchors.verticalCenter: parent.verticalCenter; text: "Switch"; onClicked: host.useHost(modelData.id) }
                Btn { visible: modelData.id !== "local"; anchors.verticalCenter: parent.verticalCenter; kind: "quiet"; text: "Forget"; onClicked: { host.forgetHost(modelData.id); hostsPage.win.hostsVersion++ } }
            }
        }
    }

    SettingCard {
        title: "Connect to a host on another machine"
        SettingRow {
            first: true
            title: "1. Make a pairing code there"
            hint: "On that host (its desktop app, or a phone paired as owner), open Settings > Access and make a code. Choose Owner to run everything from here, Approve to answer requests only."
        }
        SettingRow {
            title: "2. Paste its link here"
            hint: "The pairing link (https://…#pair=CODE), or the host's address with the code beside it. Both machines need to be on the same tailnet."
        }
        Item {
            Layout.fillWidth: true
            implicitHeight: form.implicitHeight + 24
            Rectangle { width: parent.width; height: 1; color: theme.line }
            ColumnLayout {
                id: form
                x: 16; y: 12
                width: parent.width - 32
                spacing: 8
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    Field { id: link; Layout.fillWidth: true; placeholderText: "https://host.example.ts.net/#pair=CODE" }
                    Field { id: code; Layout.preferredWidth: 120; placeholderText: "Code"; visible: link.text.indexOf("#pair=") < 0 }
                    Btn {
                        kind: "primary"
                        text: hostsPage.win && hostsPage.win.pairState === "pairing" ? "Pairing…" : "Pair"
                        enabled: link.text.trim() !== "" && !(hostsPage.win && hostsPage.win.pairState === "pairing")
                        onClicked: { hostsPage.win.pairState = "pairing"; host.pairHost(link.text, code.text) }
                    }
                }
                Text {
                    visible: !!hostsPage.win && hostsPage.win.pairState !== "" && hostsPage.win.pairState !== "pairing"
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    text: hostsPage.win ? hostsPage.win.pairState : ""
                    color: hostsPage.win && hostsPage.win.pairState.startsWith("Paired") ? theme.text2 : theme.red
                    font.pixelSize: theme.small
                }
            }
        }
    }

    component Field: TextField {
        implicitHeight: 32
        placeholderTextColor: theme.text3
        color: theme.text
        background: Rectangle { radius: theme.radiusSmall; color: theme.field; border.color: parent.activeFocus ? "#4a4a4a" : theme.line }
    }
}
