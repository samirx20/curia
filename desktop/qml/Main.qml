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
    property bool settingsOpen: host.arg("--show") === "settings"
    property bool everConnected: false
    property var searchResults: []
    property var runSummary: null
    // Bumped when the hosts this app knows change; "" / "pairing" / the result of a pairing.
    property int hostsVersion: 0
    property string pairState: ""
    readonly property var currentTeam: current ? teams.find(t => t.id === current.team_id) || null : null
    // Opens the rail on a view; the open view's button hides it again.
    function showRail(view) {
        if (railOpen && rail.tab === view) { railOpen = false; return }
        rail.tab = view
        railOpen = true
        if (view === "files") listFiles()
        if (view === "terminal") listShells()
        if (view === "browser") listPorts()
    }
    property string message: ""
    property var settings: ({})
    property var listing: ({ git: true, files: [] })
    property var opened: null
    // The open panel over the page: "", "inbox", "usage", "picker", "newrun".
    property string panel: host.arg("--show") === "files" ? "" : host.arg("--show")
    property alias page: page
    property alias rail: rail
    property alias settingsPage: settingsPage
    function lastMerge() { return conversation.lastMerge() }
    function handBacks() { return conversation.handBacks() }

    Driver { id: driver; win: win }
    Shortcut { sequence: "Ctrl+N"; onActivated: { newRun.folder = ""; win.panel = "newrun" } }
    Shortcut { sequence: "Ctrl+K"; onActivated: win.openSearch() }
    Shortcut { sequence: "Esc"; onActivated: { if (win.panel !== "") win.panel = ""; else if (win.settingsOpen) win.settingsOpen = false } }
    function openSearch() { searchResults = []; panel = "search"; search.focusField() }

    Theme { id: theme }
    FontLoader { id: inter; source: host.assetUrl("fonts") + "InterVariable.ttf" }
    FontLoader { id: mono; source: host.assetUrl("fonts") + "JetBrainsMono.ttf" }

    HostLink {
        id: host
        onConnectedChanged: if (connected) {
            win.everConnected = true
            win.refresh()
            if (win.settingsOpen) settingsPage.refresh()
            if (win.currentId !== "") win.open(win.currentId)
            if (arg("--drive") !== "" && !driver.started) driver.start(arg("--drive"))
        }
        // Following another host: nothing from the last one stays on screen.
        onHostIdChanged: {
            win.everConnected = false
            win.currentId = ""
            win.threads = []
            win.teams = []
            win.inbox = []
            win.opened = null
            win.runSummary = null
            conversation.clear()
            win.hostsVersion++
        }
        onReply: (tag, json) => win.replied(tag, JSON.parse(json))
        onFailed: (tag, text) => { if (tag === "paired") win.pairState = text; else win.message = text }
        onEvent: json => win.pushed(JSON.parse(json))
        onClosed: reason => win.message = "The host closed the link: " + reason
        onActivity: (threadId, kind) => win.refresh()
        onTerminal: json => rail.terminal.shown(JSON.parse(json))
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
    function listFiles() { send("files", "thread_tree", { thread_id: currentId }) }
    property var shells: []
    property var ports: []
    function listPorts() { if (currentId !== "") send("ports", "thread_ports", { thread_id: currentId }) }
    function listShells() { if (currentId !== "") send("shells", "terminal_list", { thread_id: currentId }) }
    function openShell(id) { rail.terminal.current = id; send("shell:open", "terminal_open", { id: id }) }
    function openFile(path) {
        const entry = listing.files.find(f => f.path === path) || {}
        opened = { path: path, status: entry.status || null }
        const media = /\.(png|jpe?g|gif|webp|svg|mp4|mov|webm|m4v)$/i.test(path)
        if (!media) {
            send("file:read", "file_read", { thread_id: currentId, path: path })
            send("file:diff", "file_diff", { thread_id: currentId, path: path })
        }
        if (media && entry.status !== "deleted") send("file:link", "file_link", { thread_id: currentId, path: path })
        // An image that changed can be compared with its last committed version.
        if (media && entry.status === "modified") send("file:head", "file_link", { thread_id: currentId, path: path, rev: "HEAD" })
    }
    function open(id) {
        currentId = id
        opened = null
        conversation.clear()
        if (rail.tab === "files") listFiles()
        rail.terminal.current = ""
        rail.terminal.view = null
        shells = []
        if (rail.tab === "terminal") listShells()
        ports = []
        if (rail.tab === "browser") listPorts()
        send("open:" + id, "thread_open", { thread_id: id, after_seq: null })
        runSummary = null
        send("summary", "run_summary", { thread_id: id })
    }
    function replied(tag, value) {
        if (tag === "paired") {
            pairState = "Paired. Following it now."
            hostsVersion++
            host.useHost(value.id)
            return
        }
        if (tag.startsWith("s:")) {
            settingsPage.replied(tag, value)
            return
        }
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
        } else if (tag === "file:link" && opened) {
            opened = Object.assign({}, opened, { url: value.url })
        } else if (tag === "file:head" && opened) {
            opened = Object.assign({}, opened, { headUrl: value.url })
        } else if (tag === "created") {
            panel = ""
            refresh()
            open(value.id)
        } else if (tag === "search") {
            searchResults = value
        } else if (tag === "summary" && current && value) {
            runSummary = value
        } else if (tag === "ports") {
            ports = value
        } else if (tag === "shells") {
            shells = value
            if (rail.terminal.current === "" && value.length) openShell(value[value.length - 1].id)
        } else if (tag === "shell:started") {
            listShells()
            openShell(value.id)
        } else if (tag === "shell:open") {
            rail.terminal.shown(value)
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
                Btn { kind: "icon"; icon: "terminal"; chosen: win.railOpen && rail.tab === "terminal"; onClicked: win.showRail("terminal") }
                Btn { kind: "icon"; icon: "web"; chosen: win.railOpen && rail.tab === "browser"; onClicked: win.showRail("browser") }
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
            onOpenSettings: win.settingsOpen = true
            onOpenSearch: win.openSearch()
            onConnectHost: { settingsPage.section = "hosts"; win.settingsOpen = true }
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
            shells: win.shells
            ports: win.ports
            onRefreshPorts: win.listPorts()
            onTermStart: (c, r) => win.send("shell:started", "terminal_start", { thread_id: win.currentId, cols: c, rows: r })
            onTermOpen: id => win.openShell(id)
            onTermInput: (id, d) => win.send("term-input", "terminal_input", { id: id, data: d })
            onTermResize: (id, c, r) => win.send("term-input", "terminal_resize", { id: id, cols: c, rows: r })
            // The design's widths: 300 for Session and Files, 520 for a file, the terminal and the browser.
            Layout.preferredWidth: tab === "terminal" || tab === "browser" || (tab === "files" && win.opened !== null) ? 520 : 300
            Layout.fillHeight: true
            thread: win.current
            threads: win.threads
            teams: win.teams
            subagents: conversation.subagents
            summary: win.runSummary
        }
    }
    }

    Settings {
        id: settingsPage
        anchors.fill: parent
        visible: win.settingsOpen
        win: win
        onClose: win.settingsOpen = false
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
    Search {
        id: search
        visible: win.panel === "search"
        anchors.horizontalCenter: parent.horizontalCenter
        y: 90
        threads: win.threads
        teams: win.teams
        results: win.searchResults
        onQuery: text => win.send("search", "search", { query: text })
        onPick: id => { win.panel = ""; win.open(id) }
        onClose: win.panel = ""
    }

    // The host can't be reached: say so over the page, and keep trying (the bridge does).
    Rectangle {
        anchors.fill: parent
        visible: win.everConnected && !host.connected
        color: "#a0000000"
        MouseArea { anchors.fill: parent; hoverEnabled: true }
        Rectangle {
            anchors.centerIn: parent
            width: 420
            height: lost.implicitHeight + 40
            radius: theme.radius
            color: theme.raised
            border.color: theme.amber
            ColumnLayout {
                id: lost
                x: 20; y: 20
                width: parent.width - 40
                spacing: 8
                RowLayout {
                    spacing: 10
                    Icon { name: "warning"; size: 18; color: theme.amber }
                    Text { text: "Can't reach the host"; color: theme.text; font.weight: Font.DemiBold; font.pixelSize: 15 }
                }
                Text {
                    Layout.fillWidth: true; wrapMode: Text.Wrap; color: theme.text2
                    text: /10061|refused|link\.json/.test(host.status) ? "The host isn't running on this computer right now." : host.status
                }
                Text { Layout.fillWidth: true; wrapMode: Text.Wrap; text: "Trying again every few seconds. Runs on the host keep going meanwhile; nothing here is lost."; color: theme.text3; font.pixelSize: theme.small }
            }
        }
    }

    NewRun {
        id: newRun
        monoFamily: mono.name
        defaultFreedom: win.settings.default_freedom || "ask"
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
