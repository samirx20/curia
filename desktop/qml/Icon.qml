import QtQuick
import QtQuick.Shapes

// One of the design's line icons (Theme.icons), drawn from its 24-unit path at any size.
// The path itself is scaled (not the item), so icons clip like any other item.
Item {
    id: icon
    property string name: ""
    property string path: theme.icons[name] || ""
    property color color: theme.text2
    property real stroke: 1.8
    property bool filled: false
    property int size: 16
    readonly property real k: size / 24
    implicitWidth: size
    implicitHeight: size

    Shape {
        anchors.fill: parent
        preferredRendererType: Shape.CurveRenderer
        ShapePath {
            scale: Qt.size(icon.k, icon.k)
            strokeColor: icon.filled ? "transparent" : icon.color
            fillColor: icon.filled ? icon.color : "transparent"
            strokeWidth: icon.stroke * icon.k
            capStyle: ShapePath.RoundCap
            joinStyle: ShapePath.RoundJoin
            PathSvg { path: icon.path }
        }
    }
}
