import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// One row of a conversation, drawn by its kind.
Item {
    id: r
    required property string kind
    required property string key
    required property string text
    required property string extra
    required property string phase
    property string mono: ""
    readonly property var x_: JSON.parse(extra)
    signal answer(string requestId, var decision)
    signal undo(string mergeId)
    signal openThread(string id)
    height: body.implicitHeight

    // "Read 2 files, ran 1 command": a tool group's line, by what the tools did.
    function summary(steps) {
        const n = {}
        for (const s of steps) n[s.name] = (n[s.name] || 0) + 1
        const say = { Read: "read %1 file(s)", Write: "wrote %1 file(s)", Edit: "edited %1 file(s)", Bash: "ran %1 command(s)",
                      Grep: "searched %1 time(s)", Glob: "listed files %1 time(s)", Agent: "started %1 subagent(s)" }
        const parts = Object.keys(n).map(k => (say[k] || ("used " + k.replace(/^mcp__curia__/, "") + " %1×")).replace("%1", n[k]))
        const s = parts.join(", ")
        return s.charAt(0).toUpperCase() + s.slice(1)
    }
    function target(s) {
        const i = s.input || {}
        return i.file_path || i.command || i.pattern || i.description || i.title || ""
    }

    Loader {
        id: body
        width: parent.width
        sourceComponent: ({ user: userC, text: textC, thinking: thinkingC, tools: toolsC, approval: approvalC,
                            handoff: handoffC, handback: handbackC, message: messageC, notice: noticeC, failed: failedC })[r.kind] || noticeC
    }

    Component {
        id: userC
        Rectangle {
            implicitHeight: t.implicitHeight + 20
            color: theme.raised; radius: theme.radius
            Text { id: t; x: 12; y: 10; width: parent.width - 24; text: r.text; color: theme.text; wrapMode: Text.Wrap; textFormat: Text.PlainText }
        }
    }
    Component {
        id: textC
        Text { text: r.text; color: theme.text; wrapMode: Text.Wrap; textFormat: Text.MarkdownText; lineHeight: 1.25 }
    }
    Component {
        id: thinkingC
        Text { text: r.text; color: theme.text3; font.italic: true; wrapMode: Text.Wrap; maximumLineCount: 3; elide: Text.ElideRight }
    }
    Component {
        id: toolsC
        Column {
            property bool open: false
            readonly property var steps: r.x_.steps || []
            readonly property var running: steps.find(s => s.state === "running")
            spacing: 4
            Text {
                text: (parent.open ? "⌄  " : "›  ") + r.summary(parent.steps)
                color: parent.steps.some(s => s.state === "error") ? theme.red : theme.text2
                MouseArea { anchors.fill: parent; onClicked: parent.parent.open = !parent.parent.open }
            }
            Text {
                visible: !parent.open && parent.running !== undefined
                leftPadding: 18
                text: parent.running ? "◌  " + parent.running.name + "  " + r.target(parent.running) : ""
                color: theme.blue; elide: Text.ElideRight; width: r.width
            }
            Repeater {
                model: parent.open ? parent.steps : []
                delegate: Column {
                    required property var modelData
                    leftPadding: 18
                    spacing: 2
                    Text {
                        width: r.width - 18; elide: Text.ElideRight
                        text: (modelData.state === "running" ? "◌ " : modelData.state === "error" ? "✕ " : "✓ ") + modelData.name + "  " + r.target(modelData)
                        color: modelData.state === "error" ? theme.red : theme.text2
                        font.family: r.mono; font.pixelSize: theme.small
                    }
                    Text {
                        visible: modelData.output !== ""
                        width: r.width - 30; leftPadding: 12
                        text: modelData.output.split("\n").slice(0, 6).join("\n")
                        color: theme.text3; font.family: r.mono; font.pixelSize: theme.small; wrapMode: Text.WrapAnywhere
                    }
                }
            }
        }
    }
    Component {
        id: approvalC
        Card {
            accent: r.phase === "pending" ? theme.amber : theme.line
            title: r.x_.kind === "question" ? "Asks you" : "Wants to run"
            badge: r.phase === "pending" ? "● Needs you" : r.phase
            badgeColor: r.phase === "pending" ? theme.amber : (r.phase === "denied" || r.phase === "expired" ? theme.red : theme.green)
            Text {
                width: parent.width
                text: r.x_.tool_name + "  " + (r.x_.input.command || r.x_.input.file_path || (r.x_.input.questions ? r.x_.input.questions[0].question : JSON.stringify(r.x_.input)))
                color: theme.text; font.family: r.mono; wrapMode: Text.WrapAnywhere
            }
            Row {
                visible: r.phase === "pending"
                spacing: 8
                Button { text: "Allow"; onClicked: r.answer(r.key, { decision: "allow" }) }
                Button { text: "Deny"; onClicked: r.answer(r.key, { decision: "deny", message: null }) }
            }
        }
    }
    Component {
        id: handoffC
        Card {
            accent: theme.line
            title: "↗  Handed to " + r.x_.team
            badge: r.x_.branch || ""
            Text { width: parent.width; text: r.text; color: theme.text; wrapMode: Text.Wrap }
            Text { text: "Open ›"; color: theme.text2; MouseArea { anchors.fill: parent; onClicked: r.openThread(r.x_.thread_id) } }
        }
    }
    Component {
        id: handbackC
        Card {
            readonly property var m: r.x_.merge
            accent: r.x_.ok ? theme.line : theme.red
            title: "↙  Result" + (r.x_.inferred ? " (from its last message)" : "")
            badge: r.phase === "undone" ? "Merge undone" : m.result === "merged" ? "✓ Merged" : m.result === "failed" ? "Not merged" : ""
            badgeColor: m.result === "failed" ? theme.red : theme.green
            Text { width: parent.width; text: r.text; color: theme.text; wrapMode: Text.Wrap }
            Text {
                visible: m.result === "merged"
                text: (m.files || []).length + " file(s) from " + m.branch
                color: theme.text3
            }
            Text { visible: m.result === "failed"; width: parent.width; text: m.reason || ""; color: theme.red; wrapMode: Text.Wrap }
            Row {
                spacing: 8
                Button { visible: m.result === "merged" && r.phase !== "undone"; text: "Undo merge"; onClicked: r.undo(r.key) }
                Button { text: "Open team thread"; flat: true; onClicked: r.openThread(r.x_.from) }
            }
        }
    }
    Component {
        id: messageC
        Card {
            accent: theme.line
            title: "From " + (r.x_.title || "another thread")
            badge: r.phase
            Text { width: parent.width; text: r.text; color: theme.text2; wrapMode: Text.Wrap }
        }
    }
    Component {
        id: noticeC
        Text { text: r.text; color: theme.text3; wrapMode: Text.Wrap; font.pixelSize: theme.small }
    }
    Component {
        id: failedC
        Text { text: "✕  " + r.text; color: theme.red; wrapMode: Text.Wrap }
    }

    component Card: Rectangle {
        default property alias content: inner.data
        property color accent: theme.line
        property string title
        property string badge
        property color badgeColor: theme.text3
        implicitHeight: inner.implicitHeight + 28
        radius: theme.radius
        color: theme.raised
        border.color: accent
        Column {
            id: inner
            x: 14; y: 14
            width: parent.width - 28
            spacing: 8
            Item {
                width: parent.width; height: 18
                Text { text: title; color: theme.text2; font.weight: Font.DemiBold }
                Text { anchors.right: parent.right; text: badge; color: badgeColor; font.pixelSize: theme.small }
            }
        }
    }
}
