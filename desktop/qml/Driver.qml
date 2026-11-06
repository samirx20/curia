import QtQuick

// `--drive "step; step; …"`: performs the steps a person would click through, by calling the
// same functions the buttons call, against the real host. For checking the app end to end
// without someone at the screen. Steps:
//   wait <ms> · newrun <title>|<folder>|<team>|<freedom> · send <text>
//   until inbox|handback|merged|idle <timeout ms> · allow-inbox · undo-merge · open-team
//   settings <section> · panel session|files|terminal|browser · go <address> · file <path> · view changes|file|preview|source|image|compare · shell (a shell in the thread's folder) · type <line> · shot <png> · quit
//   pairhost <pairing link> · usehost <id|local> · until remote|local <timeout ms> (following that host, its threads listed)
Item {
    id: driver
    property var win
    property var steps: []
    property int at: 0
    property string waitingFor: ""
    property real deadline: 0

    property bool started: false
    function start(script) {
        started = true
        steps = script.split(";").map(s => s.trim()).filter(s => s !== "")
        at = 0
        next()
    }
    function log(text) { console.log("drive: " + text) }
    function next() {
        if (at >= steps.length) { log("done"); return }
        const step = steps[at++]
        const sp = step.indexOf(" ")
        const verb = sp < 0 ? step : step.slice(0, sp)
        const arg = sp < 0 ? "" : step.slice(sp + 1)
        log(step)
        if (verb === "wait") { pause.interval = parseInt(arg); pause.start(); return }
        if (verb === "until") {
            const parts = arg.split(" ")
            waitingFor = parts[0]
            deadline = Date.now() + parseInt(parts[1] || "60000")
            poll.start()
            return
        }
        if (verb === "newrun") {
            const f = arg.split("|")
            win.send("created", "thread_create", { thread: { title: f[0], cwd: f[1], team: f[2], model: null, effort: null, permission_mode: f[3] || "ask" } })
        } else if (verb === "send") {
            win.send("refresh-after", "thread_send", { thread_id: win.currentId, text: arg })
        } else if (verb === "allow-inbox") {
            const q = win.inbox[0]
            win.send("refresh-after", "thread_answer", { thread_id: q.thread_id, request_id: q.request_id, decision: { decision: "allow" } })
        } else if (verb === "undo-merge") {
            win.send("refresh-after", "merge_undo", { merge_id: win.lastMerge() })
        } else if (verb === "open-team") {
            const t = win.threads.find(x => x.parent_id === win.currentId)
            if (t) win.open(t.id)
        } else if (verb === "panel") {
            win.showRail(arg)
        } else if (verb === "settings") {
            win.settingsOpen = true
            win.settingsPage.section = arg || "host"
        } else if (verb === "search") {
            win.openSearch()
            win.send("search", "search", { query: arg })
        } else if (verb === "pair") {
            win.settingsPage.send("s:pair", "pair_code", { role: "approve", minutes: 5 })
        } else if (verb === "pairhost") {
            win.pairState = "pairing"
            host.pairHost(arg, "")
        } else if (verb === "usehost") {
            host.useHost(arg)
        } else if (verb === "go") {
            win.rail.browser.go(arg)
        } else if (verb === "file") {
            win.openFile(arg)
        } else if (verb === "view") {
            win.rail.files.viewer.view = arg
        } else if (verb === "shell") {
            win.send("shell:started", "terminal_start", { thread_id: win.currentId, cols: win.rail.terminal.fitCols, rows: win.rail.terminal.fitRows })
        } else if (verb === "type") {
            win.send("term-input", "terminal_input", { id: win.rail.terminal.current, data: arg + "\r" })
        } else if (verb === "shot") {
            win.page.grabToImage(r => { r.saveToFile(arg); log("saved " + arg); next() })
            return
        } else if (verb === "quit") {
            Qt.quit()
            return
        }
        pause.interval = 300
        pause.start()
    }
    function met() {
        if (waitingFor === "inbox") return win.inbox.length > 0
        if (waitingFor === "handback") return win.lastMerge() !== "" || win.handBacks() > 0
        if (waitingFor === "merged") return win.lastMerge() !== ""
        if (waitingFor === "remote") return host.connected && host.hostId !== "local" && win.threads.length > 0
        if (waitingFor === "local") return host.connected && host.hostId === "local" && win.threads.length > 0
        if (waitingFor === "idle") return win.current !== null && win.current.status === "idle"
        return true
    }
    Timer { id: pause; onTriggered: driver.next() }
    Timer {
        id: poll
        interval: 500
        repeat: true
        onTriggered: {
            if (driver.met()) { stop(); driver.next() }
            else if (Date.now() > driver.deadline) { stop(); driver.log("FAILED: gave up waiting for " + driver.waitingFor); Qt.exit(3) }
        }
    }
}
