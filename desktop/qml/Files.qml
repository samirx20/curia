import QtQuick

// The rail's Files view (docs: Design 05–09): the thread's folder as a tree, or one file open.
Item {
    id: files
    property var listing: ({ git: true, root: "", branch: null, files: [] })
    property var opened: null
    property string mono: ""
    property string base: ""
    signal openFile(string path)
    signal back()

    FileTree {
        anchors.fill: parent
        visible: files.opened === null
        listing: files.listing
        mono: files.mono
        onOpenFile: path => files.openFile(path)
    }
    property alias viewer: fileView
    FileView {
        id: fileView
        anchors.fill: parent
        visible: files.opened !== null
        opened: files.opened
        mono: files.mono
        root: files.listing.root || ""
        base: files.base
        onBack: files.back()
    }
}
