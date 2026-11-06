import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// One open file in the Files rail (docs: Design 06–09): code as Changes or File, Markdown as
// Preview or Source, an image as Image or Compare (with its last committed version), a video
// with a player. The corner button opens it in this computer's own app.
Item {
    id: viewer
    property var opened: null     // {path, status, text?, patch?, binary?, too_large?, size?, url?, headUrl?}
    property string mono: ""
    property string root: ""
    property string base: ""      // the host's web address, for file links
    property string view: ""
    signal back()

    readonly property string path: opened ? opened.path : ""
    readonly property string kind: /\.(png|jpe?g|gif|webp|svg)$/i.test(path) ? "image"
                                 : /\.(mp4|mov|webm|m4v)$/i.test(path) ? "video"
                                 : /\.(md|markdown)$/i.test(path) ? "markdown" : "code"
    readonly property var views: ({ image: ["image", "compare"], markdown: ["preview", "source"], code: ["changes", "file"], video: [] })[kind]
    readonly property string shown: views.indexOf(view) >= 0 ? view : (views[0] || "")
    onPathChanged: view = ""

    // A unified diff as rows: kind (add/del/ctx/hunk), the new file's line number, text.
    function diffRows(patch) {
        const rows = []
        let n = 0
        for (const line of (patch || "").split("\n")) {
            if (/^(diff --git|index |--- |\+\+\+ |new file|deleted file|similarity|rename )/.test(line)) continue
            const h = /^@@ -\d+(?:,\d+)? \+(\d+)/.exec(line)
            if (h) { n = parseInt(h[1]); rows.push({ k: "hunk", n: "", t: line }); continue }
            if (line.startsWith("+")) rows.push({ k: "add", n: n++, t: line.slice(1) })
            else if (line.startsWith("-")) rows.push({ k: "del", n: "", t: line.slice(1) })
            else if (line !== "\\ No newline at end of file") rows.push({ k: "ctx", n: n++, t: line.slice(1) })
        }
        return rows
    }
    function fileRows(text) { return (text || "").split("\n").map((t, i) => ({ k: "ctx", n: i + 1, t: t })) }
    function sizeText(b) { return b > 1048576 ? (b / 1048576).toFixed(1) + " MB" : b > 1024 ? Math.round(b / 1024) + " KB" : b + " bytes" }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        RowLayout {
            Layout.fillWidth: true
            Layout.bottomMargin: 6
            spacing: 4
            Btn { kind: "quiet"; icon: "back"; text: "Files"; onClicked: viewer.back() }
            Text { text: "/"; color: theme.text3 }
            Text { Layout.fillWidth: true; text: viewer.path; elide: Text.ElideLeft; color: theme.text; font.family: viewer.mono; font.weight: Font.DemiBold; font.pixelSize: theme.small }
            Btn { kind: "icon"; icon: "close"; small: true; onClicked: viewer.back() }
        }
        RowLayout {
            Layout.fillWidth: true
            Layout.bottomMargin: 8
            spacing: 6
            Text {
                Layout.fillWidth: true
                elide: Text.ElideRight
                color: theme.text3; font.pixelSize: theme.tiny
                text: !viewer.opened ? "" : ({ added: "New file", modified: "Changed", deleted: "Deleted" }[viewer.opened.status] || "Unchanged")
                      + (viewer.opened.size ? " · " + viewer.sizeText(viewer.opened.size) : "")
            }
            Rectangle {
                visible: viewer.views.length > 0
                implicitWidth: seg.implicitWidth + 4; implicitHeight: 28; radius: 7; color: theme.field
                Row {
                    id: seg
                    anchors.centerIn: parent
                    spacing: 2
                    Repeater {
                        model: viewer.views
                        delegate: Btn {
                            required property string modelData
                            kind: "quiet"; small: true
                            chosen: viewer.shown === modelData
                            width: implicitWidth + 12
                            text: modelData.charAt(0).toUpperCase() + modelData.slice(1)
                            enabled: modelData !== "compare" || !!(viewer.opened && viewer.opened.headUrl)
                            onClicked: viewer.view = modelData
                        }
                    }
                }
            }
            // This computer's own app for the file (the host runs here).
            Btn { kind: "icon"; icon: "external"; small: true; onClicked: Qt.openUrlExternally("file:///" + (viewer.root + "/" + viewer.path).replace(/\\/g, "/")) }
        }

        // Code and Markdown source: numbered lines; changes coloured.
        ListView {
            visible: (viewer.kind === "code" || (viewer.kind === "markdown" && viewer.shown === "source")) && viewer.opened && !viewer.opened.binary && !viewer.opened.too_large
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}
            model: !visible ? [] : viewer.shown === "changes" ? viewer.diffRows(viewer.opened.patch) : viewer.fileRows(viewer.opened.text)
            delegate: Rectangle {
                required property var modelData
                width: ListView.view.width
                height: Math.max(19, line.implicitHeight + 2)
                color: modelData.k === "add" ? "#16301f" : modelData.k === "del" ? "#3a1c1c" : "transparent"
                Text { x: 4; width: 34; horizontalAlignment: Text.AlignRight; text: modelData.n; color: theme.text3; font.family: viewer.mono; font.pixelSize: theme.small }
                Text { x: 44; text: modelData.k === "add" ? "+" : modelData.k === "del" ? "−" : ""; color: modelData.k === "add" ? theme.green : theme.red; font.family: viewer.mono; font.pixelSize: theme.small }
                Text {
                    id: line
                    x: 58; width: parent.width - 62
                    text: modelData.t
                    textFormat: Text.PlainText
                    wrapMode: Text.WrapAnywhere
                    color: modelData.k === "hunk" ? theme.blue : theme.text
                    font.family: viewer.mono; font.pixelSize: theme.small
                }
            }
        }
        // Markdown preview.
        ScrollView {
            visible: viewer.kind === "markdown" && viewer.shown === "preview"
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            TextArea {
                readOnly: true
                wrapMode: TextArea.Wrap
                textFormat: TextEdit.MarkdownText
                text: viewer.opened && viewer.opened.text !== undefined ? viewer.opened.text : "Loading…"
                color: theme.text
                background: null
            }
        }
        // An image, or its last committed version above the one now.
        Flickable {
            visible: viewer.kind === "image"
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            contentHeight: pics.implicitHeight
            Column {
                id: pics
                width: parent.width
                spacing: 10
                Repeater {
                    model: !viewer.opened || viewer.kind !== "image" ? [] : viewer.shown === "compare" ? [["Last commit", viewer.opened.headUrl], ["Now", viewer.opened.url]] : [["", viewer.opened.url]]
                    delegate: Column {
                        required property var modelData
                        width: pics.width
                        spacing: 6
                        Rectangle {
                            width: parent.width
                            height: Math.min(img.implicitHeight * width / Math.max(1, img.implicitWidth), 600) || 200
                            radius: 8; color: theme.well; clip: true
                            Image { id: img; anchors.fill: parent; anchors.margins: 1; fillMode: Image.PreserveAspectFit; source: modelData[1] ? viewer.base + modelData[1] : ""; asynchronous: true; smooth: true; mipmap: true }
                            Text { anchors.centerIn: parent; visible: img.status === Image.Error; text: "Couldn't load the image."; color: theme.text3 }
                        }
                        Text { visible: modelData[0] !== ""; text: modelData[0] + " · " + viewer.path; color: theme.text3; font.pixelSize: theme.tiny }
                    }
                }
            }
        }
        // A video: its player lives in its own file, loaded only when needed.
        Loader {
            active: viewer.kind === "video" && viewer.opened !== null && !!viewer.opened.url
            visible: active
            Layout.fillWidth: true
            Layout.fillHeight: true
            source: "VideoView.qml"
            onLoaded: item.source = viewer.base + viewer.opened.url
        }
        // A binary or very large file that isn't a picture or video.
        Text {
            visible: viewer.kind !== "image" && viewer.kind !== "video" && viewer.opened !== null && (viewer.opened.binary || viewer.opened.too_large)
            Layout.fillWidth: true
            Layout.topMargin: 30
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            text: viewer.opened ? (viewer.opened.binary ? "A binary file" : "Too large to show here") + (viewer.opened.size ? " (" + viewer.sizeText(viewer.opened.size) + ")" : "") + ". Use the corner button to open it in its own app." : ""
            color: theme.text3
        }
        Item { visible: viewer.kind === "video" && !(viewer.opened && viewer.opened.url); Layout.fillHeight: true }
    }
}
