import QtQuick
import QtQuick.Layouts

// A titled group of setting rows on one card, with dividers between rows (docs: Design 13).
ColumnLayout {
    id: card
    default property alias rows: body.data
    property string title
    property string note
    Layout.fillWidth: true
    spacing: 8
    RowLayout {
        visible: card.title !== ""
        Layout.fillWidth: true
        Layout.topMargin: 14
        spacing: 8
        Text { text: card.title; color: theme.text; font.weight: Font.DemiBold }
        Text { text: card.note; color: theme.text3; font.pixelSize: theme.tiny; Layout.fillWidth: true }
    }
    Rectangle {
        Layout.fillWidth: true
        implicitHeight: body.implicitHeight
        radius: theme.radius
        color: theme.card
        border.color: theme.line
        ColumnLayout {
            id: body
            width: parent.width
            spacing: 0
        }
    }
}
