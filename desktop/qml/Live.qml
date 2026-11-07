import QtQuick
import Curia.Desktop

// `curia-desktop --live <qml folder>` (src/live.rs): opens Main.qml from that folder and opens
// it again, from a fresh copy, whenever a file there is saved. The open thread, the panels and
// the window's place carry over. A file with an error keeps the last good window and says why.
QtObject {
    id: live
    property var win: null
    property string seen: ""
    readonly property LiveQml files: LiveQml {}

    function reload() {
        const url = files.snapshot()
        if (url === "") return
        const component = Qt.createComponent(url)
        if (component.status !== Component.Ready) {
            console.error("live: not reloaded, " + files.toSource(component.errorString()))
            return
        }
        const old = win
        const keep = old ? {
            x: old.x, y: old.y, width: old.width, height: old.height,
            currentId: old.currentId, sideOpen: old.sideOpen, railOpen: old.railOpen,
            settingsOpen: old.settingsOpen, panel: old.panel
        } : {}
        const next = component.createObject(null, keep)
        if (!next) {
            console.error("live: not reloaded, the window could not be made")
            return
        }
        // Set again once shown: given only at creation, Qt adds the frame Windows would draw.
        if (old) { next.x = old.x; next.y = old.y; next.width = old.width; next.height = old.height }
        if (old && old.visibility === Window.Maximized) next.showMaximized()
        files.windowOpened()
        if (old) old.destroy()
        win = next
        console.log("live: reloaded " + url)
    }

    readonly property Timer poll: Timer {
        interval: 300
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: {
            const now = live.files.stamp()
            if (now === live.seen) return
            live.seen = now
            live.reload()
        }
    }
}
