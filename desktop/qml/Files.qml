import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// The rail's Files view (docs: Design 05–07): the thread's changed files, and a file opened
// as its changes, its text, or (for Markdown) a preview.
Item {
    id: files
    property var listing: ({ git: true, files: [] })
    property var opened: null     // {path, text?, patch?, binary?, too_large?}
    property string view: "changes"
    property string mono: ""
    signal openFile(string path)
    signal back()

    ColumnLayout {
        anchors.fill: parent
        spacing: 8
        visible: files.opened === null
        RowLayout {
            Layout.fillWidth: true
            Text { text: "Changed files"; color: theme.text3; font.pixelSize: theme.small; Layout.fillWidth: true }
            Text { text: files.listing.files.length; color: theme.text3; font.pixelSize: theme.small }
        }
        ColumnLayout {
            visible: !files.listing.git || files.listing.files.length === 0
            Layout.fillWidth: true
            Layout.topMargin: 40
            spacing: 8
            Icon { Layout.alignment: Qt.AlignHCenter; name: "folder"; size: 28; stroke: 1.5; color: theme.text3 }
            Text { Layout.fillWidth: true; horizontalAlignment: Text.AlignHCenter; text: files.listing.git ? "No changes" : "Not a git repository"; color: theme.text2 }
            Text {
                Layout.fillWidth: true; Layout.leftMargin: 16; Layout.rightMargin: 16
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                text: files.listing.git ? "Files the thread changes since the last commit show here." : "Changes can only be listed in a git repository."
                color: theme.text3; font.pixelSize: theme.small
            }
        }
        ListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: files.listing.files
            delegate: Rectangle {
                required property var modelData
                width: ListView.view.width
                height: 28
                radius: theme.radiusSmall
                color: area.containsMouse ? theme.raised : "transparent"
                Icon {
                    id: fileIcon
                    x: 6; anchors.verticalCenter: parent.verticalCenter
                    size: 14; color: theme.text3
                    name: /\.(png|jpe?g|gif|webp|svg)$/i.test(modelData.path) ? "image" : /\.(mp4|mov|webm)$/i.test(modelData.path) ? "video" : "file"
                }
                Text {
                    anchors.left: fileIcon.right; anchors.leftMargin: 8; anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - 110; elide: Text.ElideLeft
                    text: modelData.path; color: theme.text; font.family: files.mono; font.pixelSize: theme.small
                }
                Text {
                    anchors.right: parent.right; anchors.rightMargin: 6; anchors.verticalCenter: parent.verticalCenter
                    text: modelData.status === "added" ? "new" : modelData.status === "deleted" ? "deleted" : "+" + modelData.added + " −" + modelData.removed
                    color: modelData.status === "deleted" ? theme.red : theme.green; font.pixelSize: theme.small
                }
                MouseArea { id: area; anchors.fill: parent; hoverEnabled: true; onClicked: files.openFile(modelData.path) }
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 8
        id: viewer
        visible: files.opened !== null
        readonly property bool markdown: files.opened !== null && /\.(md|markdown)$/i.test(files.opened.path)
        RowLayout {
            Layout.fillWidth: true
            Btn { kind: "quiet"; icon: "back"; text: "Files"; onClicked: files.back() }
            Text { text: files.opened ? files.opened.path : ""; color: theme.text; elide: Text.ElideLeft; Layout.fillWidth: true; horizontalAlignment: Text.AlignRight; font.family: files.mono; font.pixelSize: theme.small }
        }
        // Which way to see it: a small segmented switch.
        Rectangle {
            implicitWidth: seg.implicitWidth + 4
            implicitHeight: 28
            radius: 7
            color: theme.field
            Row {
                id: seg
                anchors.centerIn: parent
                spacing: 2
                Repeater {
                    model: viewer.markdown ? ["preview", "source"] : ["changes", "file"]
                    delegate: Btn {
                        required property string modelData
                        kind: "quiet"
                        small: true
                        chosen: files.view === modelData
                        width: implicitWidth + 12
                        text: modelData.charAt(0).toUpperCase() + modelData.slice(1)
                        onClicked: files.view = modelData
                    }
                }
            }
        }
        ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            TextArea {
                readOnly: true
                wrapMode: TextArea.WrapAnywhere
                color: theme.text
                background: null
                font.family: files.view === "preview" ? "" : files.mono
                font.pixelSize: theme.small
                textFormat: files.view === "preview" ? TextEdit.MarkdownText : TextEdit.PlainText
                text: {
                    const o = files.opened
                    if (!o) return ""
                    if (o.binary) return "A binary file (" + o.size + " bytes)."
                    if (o.too_large) return "Too large to show here (" + o.size + " bytes)."
                    if (files.view === "changes") return o.patch === undefined ? "Loading…" : (o.patch || "No changes against the last commit.")
                    return o.text === undefined ? "Loading…" : o.text
                }
            }
        }
    }
}
