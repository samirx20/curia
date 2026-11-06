import QtQuick

// An on/off switch (docs: Design 13–20).
Rectangle {
    id: toggle
    property bool on: false
    signal toggled(bool on)
    implicitWidth: 36
    implicitHeight: 20
    radius: 10
    color: on ? theme.blue : theme.selection
    opacity: enabled ? 1 : 0.45
    Rectangle {
        x: toggle.on ? parent.width - width - 3 : 3
        y: 3; width: 14; height: 14; radius: 7
        color: theme.text
        Behavior on x { NumberAnimation { duration: 120 } }
    }
    MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: toggle.toggled(!toggle.on) }
}
