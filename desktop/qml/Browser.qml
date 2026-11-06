import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// The rail's Browser view (docs: Design 12): pages the run's programs serve, inside the app.
// The ports listed are the ones the thread's CLI, shells and what they started listen on.
Item {
    id: browser
    property var thread: null
    property var ports: []        // [{port, pid, program}]
    property string address: ""
    property string mono: ""
    signal refreshPorts()

    function go(text) {
        let u = text.trim()
        if (u === "") return
        if (/^\d+$/.test(u)) u = "http://localhost:" + u + "/"
        else if (!/^[a-z]+:\/\//i.test(u)) u = "http://" + u
        address = u
        field.text = u
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // Back, forward, reload, the address, and "open in your browser".
        RowLayout {
            Layout.fillWidth: true
            Layout.margins: 8
            spacing: 4
            Btn { kind: "icon"; small: true; icon: "back"; enabled: page.item !== null && page.item.canGoBack; onClicked: page.item.goBack() }
            Btn { kind: "icon"; small: true; icon: "open"; enabled: page.item !== null && page.item.canGoForward; onClicked: page.item.goForward() }
            Btn { kind: "icon"; small: true; icon: "undo"; enabled: browser.address !== ""; onClicked: page.item ? page.item.reload() : null }
            TextField {
                id: field
                Layout.fillWidth: true
                implicitHeight: 28
                placeholderText: "localhost:5173, or pick a port below"
                placeholderTextColor: theme.text3
                color: theme.text
                font.family: browser.mono
                font.pixelSize: theme.small
                onAccepted: browser.go(text)
                background: Rectangle { radius: theme.radiusSmall; color: theme.field; border.color: parent.activeFocus ? "#4a4a4a" : theme.line }
            }
            Btn { kind: "icon"; small: true; icon: "external"; enabled: browser.address !== ""; onClicked: Qt.openUrlExternally(browser.address) }
        }
        // The run's ports.
        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 10
            Layout.rightMargin: 8
            Layout.bottomMargin: 8
            spacing: 6
            Text { text: "Ports on this computer"; color: theme.text3; font.pixelSize: theme.tiny }
            Repeater {
                model: browser.ports
                delegate: Btn {
                    required property var modelData
                    small: true
                    chosen: browser.address.indexOf(":" + modelData.port + "/") >= 0
                    text: modelData.port + " · " + modelData.program.replace(/\.exe$/i, "")
                    onClicked: browser.go(String(modelData.port))
                }
            }
            Text { visible: browser.ports.length === 0; text: "none yet"; color: theme.text3; font.pixelSize: theme.tiny }
            Item { Layout.fillWidth: true }
            Btn { kind: "icon"; small: true; icon: "undo"; onClicked: browser.refreshPorts() }
        }
        Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: theme.line }

        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Loader {
                id: page
                anchors.fill: parent
                active: browser.address !== ""
                source: "WebPage.qml"
                onLoaded: item.url = browser.address
                Connections {
                    target: page.item
                    ignoreUnknownSignals: true
                    function onOpened(u) { if (u !== "about:blank") field.text = u }
                }
            }
            Connections {
                target: browser
                function onAddressChanged() { if (page.item) page.item.url = browser.address }
            }
            Text {
                anchors.centerIn: parent
                visible: page.status === Loader.Error
                width: parent.width - 40
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                text: "This computer's web view isn't available. Use the corner button to open the page in your browser."
                color: theme.text3
            }
            ColumnLayout {
                visible: browser.address === ""
                anchors.centerIn: parent
                spacing: 10
                Icon { Layout.alignment: Qt.AlignHCenter; name: "web"; size: 30; stroke: 1.5; color: theme.text3 }
                Text { Layout.alignment: Qt.AlignHCenter; text: browser.ports.length ? "Pick a port above to open what it serves." : "When the run starts a server, its port shows above."; color: theme.text2 }
            }
        }
    }
}
