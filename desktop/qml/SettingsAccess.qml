import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// Settings > Access and devices (docs: Design 14): how devices reach the host, the paired
// devices and their roles, and pairing a new one by QR code.
ColumnLayout {
    id: access
    property var page
    property string newRole: "approve"
    readonly property var info: page ? page.info : ({})
    readonly property var pairing: page ? page.pairing : null
    readonly property var roles: [["owner", "Owner"], ["approve", "Can approve"], ["view", "View only"]]
    property int secondsLeft: 0
    spacing: 4

    function newCode() { page.send("s:pair", "pair_code", { role: newRole, minutes: 5 }) }
    Timer {
        running: access.pairing !== null && access.visible
        interval: 1000; repeat: true; triggeredOnStart: true
        onTriggered: {
            access.secondsLeft = Math.max(0, Math.round((access.pairing.expires_at_ms - Date.now()) / 1000))
            if (access.secondsLeft === 0) access.newCode()
        }
    }

    SettingCard {
        title: "How devices reach this host"
        note: "only these addresses are open"
        SettingRow {
            first: true
            title: "This computer"
            hint: (access.info.local_url || "") + " · only programs on this computer"
            Btn { text: "Copy address"; small: true; onClicked: { clip.text = access.info.local_url || ""; clip.selectAll(); clip.copy() } }
        }
        SettingRow {
            title: "Tailscale"
            hint: access.info.remote_url ? access.info.remote_url + " · certificate from Tailscale, renewed automatically" : "Not set up. Phones reach the host through Tailscale."
            Btn { visible: !!access.info.remote_url; text: "Copy address"; small: true; onClicked: { clip.text = access.info.remote_url; clip.selectAll(); clip.copy() } }
        }
    }
    TextEdit { id: clip; visible: false }

    SettingCard {
        title: "Paired devices"
        note: access.page ? access.page.devices.length + (access.page.devices.length === 1 ? " device" : " devices") : ""
        Text {
            visible: !access.page || access.page.devices.length === 0
            Layout.margins: 16
            text: "No devices yet. Pair one below."
            color: theme.text3
        }
        Repeater {
            model: access.page ? access.page.devices : []
            delegate: SettingRow {
                required property var modelData
                required property int index
                first: index === 0
                title: modelData.name
                hint: modelData.last_seen_at ? "seen " + theme.ago(modelData.last_seen_at) + " ago" : "never signed in"
                Segmented {
                    anchors.verticalCenter: parent.verticalCenter
                    options: access.roles
                    value: modelData.role
                    onPicked: v => access.page.send("s:changed", "device_set_role", { device_id: modelData.id, role: v })
                }
                Btn { anchors.verticalCenter: parent.verticalCenter; small: true; kind: "quiet"; text: "Unpair"; onClicked: access.page.send("s:changed", "device_revoke", { device_id: modelData.id }) }
            }
        }
    }

    SettingCard {
        title: "Pair a device"
        note: "phone, tablet or another computer"
        Item {
            Layout.fillWidth: true
            implicitHeight: 160
            RowLayout {
                anchors.fill: parent
                anchors.margins: 16
                spacing: 18
                // The QR code the host made for the pairing address.
                Rectangle {
                    width: 128; height: 128; radius: 8
                    color: "white"
                    visible: !!(access.pairing && access.pairing.qr)
                    Grid {
                        anchors.centerIn: parent
                        readonly property int n: access.pairing && access.pairing.qr ? access.pairing.qr.size : 1
                        readonly property real cell: Math.floor(112 / n)
                        columns: n
                        Repeater {
                            model: access.pairing && access.pairing.qr ? access.pairing.qr.rows.join("").split("") : []
                            delegate: Rectangle { required property string modelData; width: parent.cell; height: parent.cell; color: modelData === "1" ? "black" : "white" }
                        }
                    }
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 6
                    Text {
                        visible: !access.pairing
                        Layout.fillWidth: true
                        wrapMode: Text.Wrap
                        text: "Make a one-time code, then scan it with the phone's camera or open the address on it."
                        color: theme.text2
                    }
                    Btn { visible: !access.pairing; kind: "primary"; text: "Make a pairing code"; onClicked: access.newCode() }
                    Text { visible: !!access.pairing; text: access.pairing ? "Scan with the phone's camera, or open the address on it." : ""; color: theme.text }
                    RowLayout {
                        visible: !!access.pairing
                        spacing: 10
                        Text { text: access.pairing ? access.pairing.code : ""; color: theme.text; font.pixelSize: 20; font.letterSpacing: 2; font.family: "JetBrains Mono" }
                        Text { text: "changes in " + Math.floor(access.secondsLeft / 60) + ":" + String(access.secondsLeft % 60).padStart(2, "0"); color: theme.text3; font.pixelSize: theme.small }
                    }
                    Text { visible: !!access.pairing; Layout.fillWidth: true; elide: Text.ElideMiddle; text: access.pairing ? (access.pairing.url || "Phones need the host on Tailscale first.") : ""; color: theme.text2; font.pixelSize: theme.small; font.family: "JetBrains Mono" }
                    Text { visible: !!access.pairing; Layout.fillWidth: true; wrapMode: Text.Wrap; text: "After pairing, the phone offers Add to Home Screen, so it runs like an app and can get notifications."; color: theme.text3; font.pixelSize: theme.small }
                }
            }
        }
        SettingRow {
            title: "New device gets"
            hint: "You can change it later per device."
            Segmented { options: access.roles; value: access.newRole; onPicked: v => { access.newRole = v; if (access.pairing) access.newCode() } }
        }
    }
}
