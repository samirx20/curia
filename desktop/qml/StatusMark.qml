import QtQuick

// A thread's or a step's state as the design draws it: a turning arc while running, a check
// when done, an amber dot when it needs you, a red cross when it failed, bars when paused.
Item {
    id: mark
    property string state_: "done"   // running | done | needs | failed | paused | idle
    property int size: 14
    property color tint: theme.text2
    implicitWidth: size
    implicitHeight: size

    Icon {
        visible: mark.state_ === "running"
        anchors.fill: parent
        size: mark.size; stroke: 2.4; color: theme.track; name: "ring"
    }
    Icon {
        id: glyph
        anchors.fill: parent
        size: mark.size
        stroke: 2.4
        name: ({ running: "arc", done: "check", needs: "dot", failed: "close", paused: "pause", idle: "" })[mark.state_] || ""
        filled: mark.state_ === "needs"
        color: mark.state_ === "needs" ? theme.amber : mark.state_ === "failed" ? theme.red : mark.state_ === "done" ? theme.text3 : mark.tint
        transformOrigin: Item.Center
        RotationAnimator on rotation {
            running: mark.state_ === "running" && mark.visible
            from: 0; to: 360; duration: 900; loops: Animation.Infinite
        }
    }
}
