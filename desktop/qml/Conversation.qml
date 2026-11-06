import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// One thread's conversation, built from the host's events. Consecutive tool calls fold into
// one group row (docs: Design 21–24); streamed text grows in place.
Rectangle {
    id: conv
    property var thread: null
    property var teams: []
    property var threads: []
    property string mono: ""
    property var subagents: []
    signal sendText(string text)
    signal answer(string requestId, var decision)
    signal undo(string mergeId)
    signal openThread(string id)
    signal stop()
    signal toggleRail()
    signal openPicker()

    color: theme.panel
    radius: theme.radius

    ListModel { id: rows }
    property var where: ({})   // item / request / merge id → row index

    // The newest merge still standing in this thread ("" if none), and how many results came back.
    function lastMerge() {
        for (let i = rows.count - 1; i >= 0; i--) {
            const r = rows.get(i)
            if (r.kind === "handback" && r.key !== "" && r.phase === "merged") return r.key
        }
        return ""
    }
    function handBacks() { let n = 0; for (let i = 0; i < rows.count; i++) if (rows.get(i).kind === "handback") n++; return n }
    function clear() { rows.clear(); where = {}; subagents = [] }
    function load(events) { clear(); for (const e of events) add(e); list.positionViewAtEnd() }
    function row(kind, key, text, extra, state) {
        rows.append({ kind: kind, key: key || "", text: text || "", extra: JSON.stringify(extra || {}), phase: state || "" })
        if (key) where[key] = rows.count - 1
    }
    function titleOf(id) { const t = threads.find(x => x.id === id); return t ? "\"" + t.title + "\"" : "another thread" }
    function at(key) { return where[key] === undefined ? -1 : where[key] }
    function setExtra(i, f) { const x = JSON.parse(rows.get(i).extra); f(x); rows.setProperty(i, "extra", JSON.stringify(x)) }

    function add(w) {
        const e = w.event
        switch (e.type) {
        case "turn_started":
            if (e.by === "curia") row("notice", "", "▸  Curia: " + (e.prompt || ""))
            else if (e.by) row("notice", "", "▸  Started by " + titleOf(e.by))
            else if (e.prompt === null) row("notice", "", "▸  The agent carried on by itself")
            else row("user", "", e.prompt)
            break
        case "text_started": row(e.kind === "thinking" ? "thinking" : "text", e.item, e.text); break
        case "text_delta": { const i = at(e.item); if (i >= 0) rows.setProperty(i, "text", rows.get(i).text + e.text); break }
        case "text_done": { const i = at(e.item); if (i >= 0) rows.setProperty(i, "text", e.text); else row("text", e.item, e.text); break }
        case "tool_call": {
            const step = { item: e.item, name: e.name, input: e.input, state: "running", output: "", sub: e.parent_tool_use_id !== null }
            const last = rows.count - 1
            if (last >= 0 && rows.get(last).kind === "tools") {
                setExtra(last, x => x.steps.push(step))
                where[e.item] = last
            } else row("tools", e.item, "", { steps: [step] })
            break
        }
        case "tool_result": {
            const i = at(e.item)
            if (i >= 0) setExtra(i, x => { for (const s of x.steps) if (s.item === e.item) { s.state = e.is_error ? "error" : "done"; s.output = e.output } })
            break
        }
        case "approval_requested": row("approval", e.request_id, "", e, "pending"); break
        case "approval_resolved": { const i = at(e.request_id); if (i >= 0) rows.setProperty(i, "phase", e.resolution); break }
        case "handed_off": row("handoff", e.thread_id, e.title, e); break
        case "hand_back": row("handback", e.merge.merge_id || "", e.summary, e, e.merge.result); break
        case "merge_undone": { const i = at(e.merge_id); if (i >= 0) rows.setProperty(i, "phase", "undone"); break }
        case "message": {
            // A result's own card already says it: don't repeat it as a message.
            const last = rows.count - 1
            const repeat = last >= 0 && rows.get(last).kind === "handback" && JSON.parse(rows.get(last).extra).from === e.from
            if (e.from !== null && !repeat) row("message", "", e.text, Object.assign({ title: titleOf(e.from) }, e), e.how)
            break
        }
        case "notice": row("notice", "", e.text); break
        case "turn_finished":
            if (e.outcome.outcome === "interrupted") row("notice", "", "Stopped.")
            else if (e.outcome.outcome === "failed") row("failed", "", e.outcome.message)
            else if (e.outcome.outcome === "usage_limited") row("notice", "", "The team ran out of quota.")
            break
        case "subagent_started": subagents = subagents.concat([{ id: e.task_id, name: e.description, done: false }]); break
        case "subagent_ended": subagents = subagents.map(s => s.id === e.task_id ? Object.assign({}, s, { done: true }) : s); break
        }
        if (list.atYEnd || list.count < 3) list.positionViewAtEnd()
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // The bar: status, where, and the thread's actions.
        RowLayout {
            Layout.fillWidth: true
            Layout.margins: 12
            spacing: 10
            Text {
                text: !conv.thread ? "" : (conv.thread.status === "running" ? "◌  Working" : "✓  Idle")
                color: conv.thread && conv.thread.status === "running" ? theme.blue : theme.text2
                font.weight: Font.DemiBold
            }
            Text {
                Layout.fillWidth: true
                elide: Text.ElideMiddle
                color: theme.text3
                text: !conv.thread ? "Pick a thread on the left." :
                      (conv.thread.parent_id ? "Team thread" : "Main thread") + " · " + conv.thread.freedom + " · " + conv.thread.cwd
            }
            Button {
                visible: conv.thread !== null && conv.thread.status === "running"
                text: "■  Stop"
                onClicked: conv.stop()
            }
            Button { text: "Panel"; flat: true; onClicked: conv.toggleRail() }
        }
        Rectangle { Layout.fillWidth: true; height: 1; color: theme.line }

        ListView {
            id: list
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: rows
            spacing: 10
            topMargin: 16
            bottomMargin: 16
            ScrollBar.vertical: ScrollBar {}
            // The list places each row itself, so the centred column sits inside a full-width row.
            delegate: Item {
                id: slot
                required property string kind
                required property string key
                required property string text
                required property string extra
                required property string phase
                width: list.width
                height: ev.height
                EventRow {
                    id: ev
                    width: Math.min(slot.width - 48, 680)
                    anchors.horizontalCenter: parent.horizontalCenter
                    kind: slot.kind; key: slot.key; text: slot.text; extra: slot.extra; phase: slot.phase
                    mono: conv.mono
                    onAnswer: (id, d) => conv.answer(id, d)
                    onUndo: id => conv.undo(id)
                    onOpenThread: id => conv.openThread(id)
                }
            }
        }

        Composer {
            Layout.fillWidth: true
            Layout.maximumWidth: 680
            Layout.alignment: Qt.AlignHCenter
            Layout.margins: 12
            enabled: conv.thread !== null
            thread: conv.thread
            teams: conv.teams
            onSend: text => conv.sendText(text)
            onOpenPicker: conv.openPicker()
        }
    }
}
