import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import Curia.Desktop

// The main window: sidebar, conversation, session rail. Everything shown comes from the host
// over the link; actions go back as link calls.
ApplicationWindow {
    id: win
    width: 1440
    height: 900
    visible: true
    title: "Curia"
    color: theme.app
    font.family: inter.name
    font.pixelSize: theme.size

    property var threads: []
    property var teams: []
    property var inbox: []
    property string currentId: ""
    readonly property var current: threads.find(t => t.id === currentId) || null
    property bool railOpen: true
    property bool sideOpen: true
    readonly property var currentTeam: current ? teams.find(t => t.id === current.team_id) || null : null
    // Opens the rail on a view; the open view's button hides it again.
    function showRail(view) {
        if (railOpen && rail.tab === view) { railOpen = false; return }
        rail.tab = view
        railOpen = true
        if (view === "files") listFiles()
    }
    property string message: ""
    property var settings: ({})
    property var listing: ({ git: true, files: [] })
    property var opened: null
    // The open panel over the page: "", "inbox", "usage", "picker", "newrun".
    property string panel: host.arg("--show") === "files" ? "" : host.arg("--show")
    property alias page: page
    function lastMerge() { return conversation.lastMerge() }
    function handBacks() { return conversation.handBacks() }

    Driver { id: driver; win: win }

    Theme { id: theme }
    FontLoader { id: inter; source: host.assetUrl("fonts") + "InterVariable.ttf" }
    FontLoader { id: mono; source: host.assetUrl("fonts") + "JetBrainsMono.ttf" }

    HostLink {
        id: host
        onConnectedChanged: if (connected) { win.refresh(); if (arg("--drive") !== "") driver.start(arg("--drive")) }
        onReply: (tag, json) => win.replied(tag, JSON.parse(json))
        onFailed: (tag, text) => win.message = text
        onEvent: json => win.pushed(JSON.parse(json))
        onClosed: reason => win.message = "The host closed the link: " + reason
        onActivity: (threadId, kind) => win.refresh()
        Component.onCompleted: start(arg("--data"))
    }

    // One link call; the reply comes back to `replied` under `tag`.
    function send(tag, method, params) {
        host.call(tag, JSON.stringify(params === undefined ? { method: method } : { method: method, params: params }))
    }
    function refresh() {
        send("threads", "threads_list")
        send("teams", "teams_list")
        send("inbox", "inbox_list")
        send("settings", "settings_get")
        if (rail.tab === "files" && currentId !== "") listFiles()
    }
    function listFiles() { send("files", "thread_files", { thread_id: currentId }) }
    function openFile(path) {
        opened = { path: path }
        send("file:read", "file_read", { thread_id: currentId, path: path })
        send("file:diff", "file_diff", { thread_id: currentId, path: path })
    }
    function open(id) {
        currentId = id
        opened = null
        conversation.clear()
        if (rail.tab === "files") listFiles()
        send("open:" + id, "thread_open", { thread_id: id, after_seq: null })
    }
    function replied(tag, value) {
        if (tag === "threads") {
            threads = value
            const want = host.arg("--open")
            if (currentId === "" && want !== "")
                open(want === "last" ? (value.find(t => !t.parent_id) || value[0] || {}).id || "" : want)
        } else if (tag === "teams") {
            teams = value
        } else if (tag === "inbox") {
            inbox = value
        } else if (tag.startsWith("open:") && tag.slice(5) === currentId) {
            conversation.load(value.events)
            shot.start()
        } else if (tag === "settings") {
            settings = value
        } else if (tag === "files") {
            listing = value
            if (host.arg("--show") === "files" && opened === null && value.files.length) openFile(value.files[0].path)
        } else if (tag === "file:read" && opened && value.path === opened.path) {
            opened = Object.assign({}, opened, value)
        } else if (tag === "file:diff" && opened && value.path === opened.path) {
            opened = Object.assign({}, opened, { patch: value.patch })
        } else if (tag === "created") {
            panel = ""
            refresh()
            open(value.id)
        } else if (tag === "refresh-after") {
            refresh()
        }
    }
    function pushed(e) {
        if (e.thread_id === currentId)
            conversation.add(e)
        const t = e.event.type
        if (t === "turn_started" || t === "turn_finished" || t === "handed_off" || t.startsWith("approval"))
            refresh()
    }

    // Everything sits on one page item: the window's own root item can't be grabbed for --shot.
    Rectangle {
    id: page
    anchors.fill: parent
    color: theme.app
    ColumnLayout {
    anchors.fill: parent
    anchors.margins: theme.gap
    spacing: theme.gap

    // The header (docs: Design 01): sidebar, the open thread, and the rail's views.
    RowLayout {
        Layout.fillWidth: true
        Layout.preferredHeight: 34
        spacing: 8
        Btn { kind: "icon"; icon: "sidebar"; chosen: win.sideOpen; onClicked: win.sideOpen = !win.sideOpen }
        TeamBadge { visible: win.current !== null; team: win.currentTeam; Layout.leftMargin: 6 }
        Text {
            Layout.leftMargin: 4
            text: win.current ? win.current.title : "Curia"
            color: theme.text
            font.weight: Font.DemiBold
            elide: Text.ElideRight
            Layout.maximumWidth: 520
        }
        Text {
            visible: win.current !== null && win.currentTeam !== null
            text: theme.who(win.currentTeam)
            color: theme.text3
            font.pixelSize: theme.small
        }
        Item { Layout.fillWidth: true }
        Rectangle {
            implicitWidth: views.implicitWidth + 6
            implicitHeight: 32
            radius: 8
            color: theme.panel
            Row {
                id: views
                anchors.centerIn: parent
                spacing: 2
                Btn { kind: "icon"; icon: "session"; chosen: win.railOpen && rail.tab === "session"; onClicked: win.showRail("session") }
                Btn { kind: "icon"; icon: "folder"; chosen: win.railOpen && rail.tab === "files"; onClicked: win.showRail("files") }
            }
        }
    }

    RowLayout {
        Layout.fillWidth: true
        Layout.fillHeight: true
        spacing: theme.gap

        Sidebar {
            visible: win.sideOpen
            Layout.preferredWidth: 260
            Layout.fillHeight: true
            threads: win.threads
            teams: win.teams
            inbox: win.inbox
            currentId: win.currentId
            connected: host.connected
            status: host.status
            onPicked: id => win.open(id)
            onOpenInbox: win.panel = "inbox"
            onOpenUsage: win.panel = "usage"
            onNewRun: folder => { newRun.folder = folder; win.panel = "newrun" }
        }
        Conversation {
            id: conversation
            Layout.fillWidth: true
            Layout.fillHeight: true
            thread: win.current
            teams: win.teams
            threads: win.threads
            mono: mono.name
            onSendText: text => win.send("refresh-after", "thread_send", { thread_id: win.currentId, text: text })
            onAnswer: (requestId, decision) => win.send("refresh-after", "thread_answer", { thread_id: win.currentId, request_id: requestId, decision: decision })
            onUndo: mergeId => win.send("refresh-after", "merge_undo", { merge_id: mergeId })
            onOpenThread: id => win.open(id)
            onStop: win.send("refresh-after", "thread_interrupt", { thread_id: win.currentId })
            onOpenPicker: win.panel = "picker"
        }
        Rail {
            id: rail
            visible: win.railOpen
            tab: host.arg("--show") === "files" ? "files" : "session"
            listing: win.listing
            opened: win.opened
            mono: mono.name
            onListFiles: win.listFiles()
            onOpenFile: path => win.openFile(path)
            onCloseFile: win.opened = null
            Layout.preferredWidth: 300
            Layout.fillHeight: true
            thread: win.current
            threads: win.threads
            teams: win.teams
            subagents: conversation.subagents
        }
    }
    }

    // Panels over the page; a click outside closes them.
    Rectangle {
        anchors.fill: parent
        visible: win.panel !== ""
        color: "#80000000"
        MouseArea { anchors.fill: parent; onClicked: win.panel = "" }
    }
    Inbox {
        visible: win.panel === "inbox"
        anchors.right: parent.right; anchors.top: parent.top; anchors.bottom: parent.bottom
        anchors.margins: theme.gap
        width: 420
        items: win.inbox
        threads: win.threads
        teams: win.teams
        mono: mono.name
        onAnswer: (threadId, requestId, decision) => win.send("refresh-after", "thread_answer", { thread_id: threadId, request_id: requestId, decision: decision })
        onOpenThread: id => { win.panel = ""; win.open(id) }
        onClose: win.panel = ""
    }
    UsagePanel {
        visible: win.panel === "usage"
        anchors.left: parent.left; anchors.bottom: parent.bottom
        anchors.leftMargin: 20; anchors.bottomMargin: 60
        width: 380
        teams: win.teams
        settings: win.settings
        onSetSetting: (key, value) => {
            const s = Object.assign({}, win.settings); s[key] = value
            win.send("settings", "settings_set", { settings: s })
        }
        onClose: win.panel = ""
    }
    Picker {
        visible: win.panel === "picker"
        x: (parent.width - width) / 2 - 120
        anchors.bottom: parent.bottom; anchors.bottomMargin: 110
        thread: win.current
        teams: win.teams
        onMoveTo: team => { win.panel = ""; win.send("refresh-after", "thread_move", { thread_id: win.currentId, team: team }) }
        onConfigure: options => { win.panel = ""; win.send("refresh-after", "thread_configure", Object.assign({ thread_id: win.currentId, model: null, effort: null, freedom: null }, options)) }
        onClose: win.panel = ""
    }
    NewRun {
        id: newRun
        monoFamily: mono.name
        visible: win.panel === "newrun"
        anchors.centerIn: parent
        teams: win.teams
        onCreate: params => win.send("created", "thread_create", { thread: params })
        onClose: win.panel = ""
    }

    // A message from the host or a failed call, shown briefly at the bottom.
    Rectangle {
        visible: win.message !== ""
        anchors.bottom: parent.bottom
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottomMargin: 24
        width: Math.min(toastText.implicitWidth + 60, parent.width - 80)
        height: toastText.implicitHeight + 20
        radius: theme.radius
        color: theme.raised
        border.color: theme.red
        Icon { id: toastIcon; x: 14; anchors.verticalCenter: parent.verticalCenter; name: "warning"; size: 15; color: theme.red }
        Text { id: toastText; anchors.left: toastIcon.right; anchors.leftMargin: 10; anchors.verticalCenter: parent.verticalCenter; width: Math.min(implicitWidth, win.width - 160); text: win.message; color: theme.text; wrapMode: Text.Wrap }
        MouseArea { anchors.fill: parent; onClicked: win.message = "" }
    }

    }

    // `--shot <png>`: a picture of the window once the thread is on screen, then quit.
    Timer {
        id: shot
        interval: 1500
        onTriggered: {
            const path = host.arg("--shot")
            if (path !== "")
                page.grabToImage(r => { r.saveToFile(path); Qt.quit() })
        }
    }
    Timer {
        interval: 6000
        running: host.arg("--shot") !== "" && host.arg("--open") === ""
        onTriggered: shot.start()
    }
}
