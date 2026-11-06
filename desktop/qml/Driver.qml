import QtQuick

// `--drive "step; step; …"`: performs the steps a person would click through, by calling the
// same functions the buttons call, against the real host. For checking the app end to end
// without someone at the screen. Steps:
//   wait <ms> · newrun <title>|<folder>|<team>|<freedom> · send <text>
//   until inbox|handback|merged|idle <timeout ms> · allow-inbox · undo-merge · open-team
//   shot <png> · quit
Item {
    id: driver
    property var win
    property var steps: []
    property int at: 0
    property string waitingFor: ""
    property real deadline: 0

    function start(script) {
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
