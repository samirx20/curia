import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// The Files rail's tree (docs: Design 05): the thread's folder with A/M/D marks, a filter, and
// "Changed only". Folders with changes start open.
Item {
    id: tree
    property var listing: ({ git: true, root: "", branch: null, files: [] })
    property string mono: ""
    property bool changedOnly: false
    property string filter: ""
    property var openFolders: ({})
    property var touched: ({})        // folders the person opened or closed by hand
    signal openFile(string path)

    function kindIcon(path) {
        if (/\.(png|jpe?g|gif|webp|svg)$/i.test(path)) return "image"
        if (/\.(mp4|mov|webm|m4v)$/i.test(path)) return "video"
        return "file"
    }
    // The rows to show: folders and files in order, with their depth.
    readonly property var rows: {
        const files = listing.files.filter(f => (!changedOnly || f.status) && (filter === "" || f.path.toLowerCase().includes(filter.toLowerCase())))
        const folders = {}
        for (const f of files) {
            const parts = f.path.split("/")
            for (let i = 1; i < parts.length; i++) {
                const dir = parts.slice(0, i).join("/")
                if (!folders[dir]) folders[dir] = { changed: false }
                if (f.status) folders[dir].changed = true
            }
        }
        const isOpen = dir => touched[dir] ? openFolders[dir] === true : (filter !== "" || folders[dir].changed)
        const entries = Object.keys(folders).map(d => ({ path: d, dir: true })).concat(files.map(f => ({ path: f.path, dir: false, status: f.status })))
        // Folders before files at each level, then by name.
        entries.sort((a, b) => {
            const pa = a.path.split("/"), pb = b.path.split("/")
            for (let i = 0; i < Math.min(pa.length, pb.length); i++) {
                if (pa[i] === pb[i]) continue
                const aDir = i < pa.length - 1 || a.dir, bDir = i < pb.length - 1 || b.dir
                if (aDir !== bDir) return aDir ? -1 : 1
                return pa[i].localeCompare(pb[i])
            }
            return pa.length - pb.length
        })
        return entries.filter(e => {
            const parts = e.path.split("/")
            for (let i = 1; i < parts.length; i++) if (!isOpen(parts.slice(0, i).join("/"))) return false
            return true
        }).map(e => Object.assign({ depth: e.path.split("/").length - 1, name: e.path.split("/").pop(), open: e.dir && isOpen(e.path) }, e))
    }
    function toggle(dir, now) {
        const o = Object.assign({}, openFolders), t = Object.assign({}, touched)
        o[dir] = !now; t[dir] = true
        openFolders = o; touched = t
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 8

        RowLayout {
            Layout.fillWidth: true
            spacing: 6
            Icon { name: "folder"; size: 14; color: theme.text3 }
            Text { Layout.fillWidth: true; text: tree.listing.root; elide: Text.ElideLeft; color: theme.text2; font.pixelSize: theme.small }
            Text { visible: !!tree.listing.branch; text: tree.listing.branch || ""; color: theme.text3; font.pixelSize: theme.small; font.family: tree.mono }
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 6
            TextField {
                Layout.fillWidth: true
                implicitHeight: 30
                placeholderText: "Filter files"
                placeholderTextColor: theme.text3
                color: theme.text
                font.pixelSize: theme.small
                onTextChanged: tree.filter = text
                background: Rectangle { radius: theme.radiusSmall; color: theme.field; border.color: parent.activeFocus ? "#4a4a4a" : theme.line }
            }
            Btn { text: "Changed only"; small: true; chosen: tree.changedOnly; border.width: tree.changedOnly ? 1 : 0; border.color: theme.blue; onClicked: tree.changedOnly = !tree.changedOnly }
        }

        ListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: tree.rows
            ScrollBar.vertical: ScrollBar {}
            delegate: Rectangle {
                required property var modelData
                width: ListView.view.width
                height: 26
                radius: theme.radiusSmall
                color: area.containsMouse ? theme.raised : "transparent"
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 4 + modelData.depth * 14
                    anchors.rightMargin: 8
                    spacing: 6
                    Item {
                        implicitWidth: 12; implicitHeight: 12
                        Icon { visible: modelData.dir; anchors.fill: parent; name: modelData.open ? "down" : "open"; size: 12; stroke: 2; color: theme.text3 }
                    }
                    Icon { name: modelData.dir ? "folder" : tree.kindIcon(modelData.path); size: 14; color: theme.text3 }
                    Text { Layout.fillWidth: true; text: modelData.name; elide: Text.ElideRight; color: modelData.status === "deleted" ? theme.text3 : theme.text; font.strikeout: modelData.status === "deleted" }
                    Text {
                        visible: !modelData.dir && !!modelData.status
                        text: ({ added: "A", modified: "M", deleted: "D" })[modelData.status] || ""
                        color: modelData.status === "added" ? theme.green : modelData.status === "deleted" ? theme.red : theme.text2
                        font.pixelSize: theme.tiny; font.weight: Font.Bold; font.family: tree.mono
                    }
                }
                MouseArea {
                    id: area
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: modelData.dir ? tree.toggle(modelData.path, modelData.open) : tree.openFile(modelData.path)
                }
            }
        }
        Text {
            visible: tree.rows.length === 0
            Layout.alignment: Qt.AlignHCenter
            Layout.bottomMargin: 40
            text: tree.listing.files.length === 0 ? "This folder is empty." : tree.changedOnly ? "No changes since the last commit." : "No file matches."
            color: theme.text3
        }
    }
}
