import QtQuick

// The design's tokens (docs: Design/README "Style used throughout").
QtObject {
    readonly property color app: "#0D0D0D"
    readonly property color panel: "#181818"
    readonly property color raised: "#1F1F1F"
    readonly property color field: "#222222"
    readonly property color selection: "#262626"
    readonly property color line: "#2A2A2A"
    readonly property color text: "#ECECEC"
    readonly property color text2: "#A8A8A8"
    readonly property color text3: "#8C8C8C"
    // Colour only where it means something.
    readonly property color blue: "#6B9BFF"    // selected or on
    readonly property color amber: "#E8B04B"   // needs you, or a warning
    readonly property color red: "#FF8F87"     // failed
    readonly property color green: "#5BC48A"   // done or added
    readonly property color purple: "#B392F0"  // an agent
    readonly property int radius: 10
    readonly property int gap: 6
    readonly property int size: 13
    readonly property int small: 12
}
