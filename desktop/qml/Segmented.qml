import QtQuick

// One choice of a few, side by side (docs: Design 15, 17). `options`: [[value, label], ...].
Rectangle {
    id: seg
    property var options: []
    property var value
    signal picked(var value)
    implicitWidth: row.implicitWidth + 4
    implicitHeight: 30
    radius: 7
    color: theme.field
    Row {
        id: row
        anchors.centerIn: parent
        spacing: 2
        Repeater {
            model: seg.options
            delegate: Btn {
                required property var modelData
                kind: "quiet"; small: true
                chosen: seg.value === modelData[0]
                width: implicitWidth + 12
                text: modelData[1]
                onClicked: seg.picked(modelData[0])
            }
        }
    }
}
