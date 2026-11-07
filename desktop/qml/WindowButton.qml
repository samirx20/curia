import QtQuick

// A window button in the header (minimize, maximize, close), in the app's icon-button style.
// Close turns red under the mouse, as Windows' own does.
Rectangle {
    id: wb
    property string icon: ""
    property bool danger: false
    signal clicked()

    implicitWidth: 38
    implicitHeight: 30
    radius: theme.radiusSmall
    color: !area.containsMouse ? "transparent" : danger ? "#C42B1C" : theme.raised

    Icon {
        anchors.centerIn: parent
        name: wb.icon
        size: 16
        stroke: 1.5
        color: area.containsMouse ? (wb.danger ? "#FFFFFF" : theme.text) : theme.text2
    }
    MouseArea {
        id: area
        anchors.fill: parent
        hoverEnabled: true
        onClicked: wb.clicked()
    }
}
