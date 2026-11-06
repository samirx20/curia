import QtQuick
import QtQuick.Layouts

// One setting: what it is on the left, its control on the right; a divider above all but the
// first row of a card.
Item {
    id: row
    default property alias control: right.data
    property string title
    property string hint
    property bool first: false
    /// A team to show the badge of, before the title.
    property var team: null
    Layout.fillWidth: true
    implicitHeight: Math.max(56, left.implicitHeight + 24, right.implicitHeight + 20)
    Rectangle { visible: !row.first; width: parent.width; height: 1; color: theme.line }
    TeamBadge {
        id: badge
        visible: row.team !== null
        team: row.team
        size: 26
        anchors.left: parent.left
        anchors.leftMargin: 16
        anchors.verticalCenter: parent.verticalCenter
    }
    ColumnLayout {
        id: left
        anchors.left: row.team !== null ? badge.right : parent.left
        anchors.leftMargin: row.team !== null ? 16 : 16
        anchors.right: right.left
        anchors.rightMargin: 16
        anchors.verticalCenter: parent.verticalCenter
        spacing: 2
        Text { text: row.title; color: theme.text; font.weight: Font.Medium; Layout.fillWidth: true; wrapMode: Text.Wrap }
        Text { visible: row.hint !== ""; text: row.hint; color: theme.text3; font.pixelSize: theme.small; Layout.fillWidth: true; wrapMode: Text.Wrap }
    }
    Row {
        id: right
        anchors.right: parent.right
        anchors.rightMargin: 16
        anchors.verticalCenter: parent.verticalCenter
        spacing: 8
    }
}
