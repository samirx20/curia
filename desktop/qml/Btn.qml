import QtQuick

// The app's one button (docs: Design: "the white button is the main action").
// `kind`: primary (white), secondary (grey), quiet (text only), icon (a square icon button).
Rectangle {
    id: btn
    property string text: ""
    property string icon: ""
    property string kind: "secondary"
    property bool chosen: false
    property bool small: false
    signal clicked()

    readonly property color fg: kind === "primary" ? theme.onLight : (chosen ? theme.text : (kind === "quiet" || kind === "icon") && !area.containsMouse ? theme.text2 : theme.text)
    implicitHeight: small ? 24 : 28
    implicitWidth: kind === "icon" ? implicitHeight : row.implicitWidth + (kind === "quiet" ? 4 : (small ? 16 : 22))
    radius: theme.radiusSmall
    opacity: enabled ? 1 : 0.45
    color: kind === "primary" ? (area.pressed ? "#D0D0D0" : theme.text)
         : kind === "secondary" ? (area.containsMouse ? "#2E2E2E" : theme.selection)
         : chosen ? theme.selection : (area.containsMouse && kind === "icon" ? theme.raised : "transparent")

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 6
        Icon {
            visible: btn.icon !== ""
            anchors.verticalCenter: parent.verticalCenter
            name: btn.icon
            size: btn.kind === "icon" ? 16 : 14
            color: btn.fg
        }
        Text {
            visible: btn.text !== ""
            anchors.verticalCenter: parent.verticalCenter
            text: btn.text
            color: btn.fg
            font.pixelSize: btn.small ? theme.small : theme.size
            font.weight: btn.kind === "primary" ? Font.DemiBold : Font.Normal
        }
    }
    MouseArea {
        id: area
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: btn.clicked()
    }
}
