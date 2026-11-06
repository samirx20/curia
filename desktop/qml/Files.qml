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
        Text { visible: !files.listing.git; text: "This folder isn't a git repository."; color: theme.text3 }
        Text { visible: files.listing.git && files.listing.files.length === 0; text: "No changes."; color: theme.text3 }
        ListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: files.listing.files
            delegate: Rectangle {
                required property var modelData
                width: ListView.view.width
                height: 26
                radius: 6
                color: area.containsMouse ? theme.raised : "transparent"
                Text {
                    x: 6; anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - 90; elide: Text.ElideLeft
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
        visible: files.opened !== null
        readonly property bool markdown: files.opened !== null && /\.(md|markdown)$/i.test(files.opened.path)
        RowLayout {
            Layout.fillWidth: true
            Text { text: "‹ Files"; color: theme.text2; MouseArea { anchors.fill: parent; onClicked: files.back() } }
            Text { text: files.opened ? files.opened.path : ""; color: theme.text; elide: Text.ElideLeft; Layout.fillWidth: true; horizontalAlignment: Text.AlignRight }
        }
        Row {
            spacing: 6
            Repeater {
                model: parent.parent.markdown ? ["preview", "source"] : ["changes", "file"]
                delegate: Rectangle {
                    required property string modelData
                    width: label.implicitWidth + 18; height: 24; radius: 12
                    color: files.view === modelData ? theme.selection : theme.field
                    Text { id: label; anchors.centerIn: parent; text: modelData.charAt(0).toUpperCase() + modelData.slice(1); color: theme.text; font.pixelSize: theme.small }
                    MouseArea { anchors.fill: parent; onClicked: files.view = modelData }
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
