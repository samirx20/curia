import QtQuick

// A team's icon (docs: Design 01): its CLI's mark on a small tile, with the team's two letters.
// `size` grows it for headers; the letters stay their design size.
Item {
    id: badge
    property var team: null
    property string cli: team ? team.cli : ""
    property string name: team ? team.name : ""
    property int size: 18
    implicitWidth: size
    implicitHeight: size

    Rectangle {
        anchors.fill: parent
        radius: Math.round(badge.size * 0.28)
        color: theme.line
        // The provider's own mark when it's installed (desktop/providers), else the line mark.
        Image {
            id: logo
            anchors.centerIn: parent
            width: Math.round(badge.size * 0.72); height: width
            sourceSize: Qt.size(64, 64)
            source: badge.cli !== "" ? host.assetUrl("providers") + badge.cli + ".png" : ""
            smooth: true
            mipmap: true
            visible: status === Image.Ready
        }
        Icon {
            visible: logo.status !== Image.Ready
            anchors.centerIn: parent
            size: Math.round(badge.size * 0.67)
            stroke: 2.2
            color: "#C8C8C8"
            path: theme.cli(badge.cli).glyph
        }
    }
    Rectangle {
        visible: badge.name !== ""
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.rightMargin: -5
        anchors.bottomMargin: -4
        width: tag.implicitWidth + 4
        height: 10
        radius: 3
        color: "#D9D9D9"
        Text {
            id: tag
            anchors.centerIn: parent
            text: theme.initials(badge.name)
            color: theme.onLight
            font.pixelSize: 8     // the design's 7.5 px, which pixelSize can't take
            scale: 0.94
            font.weight: Font.Bold
            font.letterSpacing: 0.2
        }
    }
}
