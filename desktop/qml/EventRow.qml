import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

// One row of a conversation, drawn by its kind (docs: Design 01, 21–24).
Item {
    id: r
    required property string kind
    required property string key
    required property string text
    required property string extra
    required property string phase
    property string mono: ""
    property var teams: []
    property var threads: []
    readonly property var x_: JSON.parse(extra)
    signal answer(string requestId, var decision)
    signal undo(string mergeId)
    signal openThread(string id)
    // Reasoning that came back empty (Codex sends some) takes no room.
    readonly property bool empty: (kind === "thinking" || kind === "text") && text.trim() === ""
    height: empty ? 0 : body.implicitHeight

    // "Read 2 files, ran 1 command": a tool group's line, by what the tools did.
    function summary(steps) {
        const n = {}
        for (const s of steps) n[s.name] = (n[s.name] || 0) + 1
        const say = { Read: "read %1 file(s)", Write: "wrote %1 file(s)", Edit: "edited %1 file(s)", Bash: "ran %1 command(s)", Shell: "ran %1 command(s)",
                      Grep: "searched %1 time(s)", Glob: "listed files %1 time(s)", Agent: "started %1 subagent(s)",
                      write_file: "wrote %1 file(s)", read_file: "read %1 file(s)", run_command: "ran %1 command(s)",
                      mcp__curia__report_result: "reported the result", mcp__curia__hand_off: "handed off %1 task(s)",
                      mcp__curia__send_message: "sent %1 message(s)", mcp__curia__read_messages: "read its messages",
                      mcp__curia__list_teams: "looked at the teams", mcp__curia__ask_human: "asked you %1 question(s)" }
        const parts = Object.keys(n).map(k => (say[k] || ("used " + short(k) + " %1×")).replace("%1", n[k]))
        const s = parts.join(", ")
        return s.charAt(0).toUpperCase() + s.slice(1)
    }
    function short(name) { return name.replace(/^mcp__curia__/, "").replace(/^mcp__([^_]+)__/, "$1: ") }
    function target(s) {
        const i = s.input || {}
        const t = i.file_path || i.Path || i.TargetFile || i.path || i.command || i.CommandLine || i.pattern || i.description || i.title || i.summary || ""
        return typeof t === "string" ? t : JSON.stringify(t)
    }
    // The icon for what a tool does.
    function kindIcon(name) {
        if (/^(Read|read_file|view_file)$/.test(name)) return "read"
        if (/^(Write|Edit|MultiEdit|write_file|edit_file|replace_file_content|multi_replace_file_content)$/.test(name)) return "edit"
        if (/^(Bash|Shell|run_command|BashOutput)$/.test(name)) return "shell"
        if (/^(Grep|Glob|search|grep_search|find_by_name|list_dir)$/.test(name)) return "search"
        if (/^(WebFetch|WebSearch|read_url_content|search_web)/.test(name)) return "web"
        if (/^(Agent|Task|browser_subagent)$/.test(name)) return "people"
        if (/^mcp__/.test(name)) return "mcp"
        return "tools"
    }
    function teamNamed(name) { return teams.find(t => t.name === name) || { cli: "", name: name } }
    function teamOfThread(id) {
        const t = threads.find(x => x.id === id)
        return t ? (teams.find(x => x.id === t.team_id) || { cli: t.cli, name: "" }) : null
    }
    function threadOf(id) { return threads.find(x => x.id === id) || null }

    Loader {
        id: body
        width: parent.width
        active: !r.empty
        sourceComponent: ({ user: userC, text: textC, thinking: thinkingC, tools: toolsC, approval: approvalC,
                            handoff: handoffC, handback: handbackC, message: messageC, notice: noticeC, failed: failedC })[r.kind] || noticeC
    }

    Component {
        id: userC
        Item {
            implicitHeight: bubble.height
            Rectangle {
                id: bubble
                anchors.right: parent.right
                width: Math.min(parent.width * 0.85, t.implicitWidth + 28)
                height: t.implicitHeight + 20
                color: theme.selection; radius: theme.radius
                Text {
                    id: t
                    x: 14; y: 10
                    width: Math.min(r.width * 0.85 - 28, implicitWidth)
                    text: r.text; color: theme.text; wrapMode: Text.Wrap; textFormat: Text.PlainText; lineHeight: 1.2
                }
            }
        }
    }
    Component {
        id: textC
        Text { text: r.text; color: theme.text; wrapMode: Text.Wrap; textFormat: Text.MarkdownText; lineHeight: 1.3; onLinkActivated: link => Qt.openUrlExternally(link) }
    }
    Component {
        id: thinkingC
        RowLayout {
            spacing: 8
            Icon { name: "message"; size: 13; color: theme.text3; Layout.alignment: Qt.AlignTop; Layout.topMargin: 2 }
            Text { Layout.fillWidth: true; text: r.text; color: theme.text3; font.italic: true; wrapMode: Text.Wrap; maximumLineCount: 3; elide: Text.ElideRight }
        }
    }
    Component {
        id: toolsC
        Column {
            id: group
            property bool open: false
            readonly property var steps: r.x_.steps || []
            readonly property var running: steps.find(s => s.state === "running")
            readonly property bool failed: steps.some(s => s.state === "error")
            spacing: 6
            // The group's line: what kind of work, how much, and a chevron to open it.
            Rectangle {
                width: line.implicitWidth + 12; height: 24
                radius: theme.radiusSmall
                x: -6
                color: lineArea.containsMouse ? theme.raised : "transparent"
                Row {
                    id: line
                    anchors.verticalCenter: parent.verticalCenter
                    x: 6
                    spacing: 8
                    Icon { anchors.verticalCenter: parent.verticalCenter; name: r.kindIcon(group.steps.length ? group.steps[0].name : ""); size: 14; color: group.failed ? theme.red : theme.text3 }
                    Text { anchors.verticalCenter: parent.verticalCenter; text: r.summary(group.steps); color: group.failed ? theme.red : theme.text2 }
                    Icon { anchors.verticalCenter: parent.verticalCenter; name: group.open ? "down" : "open"; size: 12; stroke: 2; color: theme.text3 }
                }
                MouseArea { id: lineArea; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: group.open = !group.open }
            }
            // While it runs and is closed: the step happening now.
            Row {
                visible: !group.open && group.running !== undefined
                leftPadding: 22
                spacing: 8
                StatusMark { anchors.verticalCenter: parent.verticalCenter; size: 12; state_: "running" }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    width: r.width - 60; elide: Text.ElideRight
                    text: group.running ? r.short(group.running.name) + "  " + r.target(group.running) : ""
                    color: theme.text2; font.pixelSize: theme.small
                }
            }
            Repeater {
                model: group.open ? group.steps : []
                delegate: Column {
                    required property var modelData
                    leftPadding: 22
                    spacing: 4
                    Row {
                        spacing: 8
                        StatusMark { anchors.verticalCenter: parent.verticalCenter; size: 12; state_: modelData.state === "running" ? "running" : modelData.state === "error" ? "failed" : "done" }
                        Text { anchors.verticalCenter: parent.verticalCenter; text: r.short(modelData.name); color: theme.text2; font.pixelSize: theme.small }
                        Chip {
                            visible: r.target(modelData) !== ""
                            anchors.verticalCenter: parent.verticalCenter
                            text: r.target(modelData)
                            mono: true
                            maxWidth: r.width - 180
                        }
                    }
                    Rectangle {
                        visible: modelData.output !== ""
                        width: r.width - 22; height: out.implicitHeight + 14
                        radius: 8
                        color: theme.well
                        Text {
                            id: out
                            x: 10; y: 7; width: parent.width - 20
                            text: modelData.output.split("\n").slice(0, 6).join("\n")
                            color: modelData.state === "error" ? theme.red : theme.text3
                            font.family: r.mono; font.pixelSize: theme.tiny; wrapMode: Text.WrapAnywhere
                        }
                    }
                }
            }
        }
    }
    Component {
        id: approvalC
        Card {
            accent: r.phase === "pending" ? theme.amber : theme.line
            icon: r.x_.kind === "question" ? "message" : "approvals"
            iconColor: r.phase === "pending" ? theme.amber : theme.text3
            lead: r.x_.kind === "question" ? "Asks you" : "Wants to run"
            who: r.short(r.x_.tool_name)
            status: r.phase === "pending" ? "needs" : (r.phase === "denied" || r.phase === "expired" || r.phase === "cancelled" ? "failed" : "done")
            statusText: ({ pending: "Needs you", allowed: "Allowed", denied: "Denied", answered: "Answered", expired: "Denied after waiting", cancelled: "Cancelled" })[r.phase] || r.phase
            statusColor: r.phase === "pending" ? theme.amber : theme.text3
            Well {
                why: r.x_.kind === "question" ? "Question" : "What it runs"
                code: r.x_.input.command || r.x_.input.file_path || (r.x_.input.questions ? r.x_.input.questions[0].question : JSON.stringify(r.x_.input))
                Row {
                    visible: r.phase === "pending"
                    spacing: 6
                    Btn { kind: "primary"; text: "Allow"; onClicked: r.answer(r.key, { decision: "allow" }) }
                    Btn { text: "Deny"; onClicked: r.answer(r.key, { decision: "deny", message: null }) }
                }
            }
        }
    }
    Component {
        id: handoffC
        Card {
            readonly property var child: r.threadOf(r.x_.thread_id)
            icon: "handOut"
            lead: "Handed to"
            team: r.teamNamed(r.x_.team)
            status: child && child.status === "running" ? "running" : "done"
            statusText: child && child.status === "running" ? "Running · " + theme.ago(child.updated_at) : "Idle"
            Text { width: parent.width; text: r.text; color: theme.text; wrapMode: Text.Wrap; lineHeight: 1.2 }
            Footer {
                Chip { visible: (r.x_.branch || "") !== ""; icon: "branch"; text: r.x_.branch || "" }
                Item { Layout.fillWidth: true }
                Btn { kind: "quiet"; text: "Open"; icon: "open"; onClicked: r.openThread(r.x_.thread_id) }
            }
        }
    }
    Component {
        id: handbackC
        Card {
            readonly property var m: r.x_.merge
            accent: r.x_.ok ? theme.line : theme.red
            icon: "handIn"
            lead: "Result from"
            team: r.teamOfThread(r.x_.from)
            status: r.x_.ok ? "done" : "failed"
            statusText: r.x_.ok ? "Done" : "Didn't finish"
            statusColor: r.x_.ok ? theme.text3 : theme.red
            Text { width: parent.width; text: r.text; color: theme.text; wrapMode: Text.Wrap; lineHeight: 1.2 }
            Text { visible: m.result === "failed"; width: parent.width; text: m.reason || ""; color: theme.red; wrapMode: Text.Wrap }
            Footer {
                Chip { visible: m.result === "merged"; icon: "file"; text: (m.files || []).length + ((m.files || []).length === 1 ? " file" : " files") }
                Chip { visible: r.x_.inferred; text: "from its last message" }
                Item { Layout.fillWidth: true }
                Row {
                    visible: m.result === "merged" || r.phase === "undone"
                    spacing: 6
                    Icon { anchors.verticalCenter: parent.verticalCenter; name: r.phase === "undone" ? "undo" : "check"; size: 13; color: r.phase === "undone" ? theme.text3 : theme.green }
                    Text { anchors.verticalCenter: parent.verticalCenter; text: r.phase === "undone" ? "Merge undone" : "Merged into the folder"; color: r.phase === "undone" ? theme.text3 : theme.green; font.pixelSize: theme.small }
                }
                Btn { visible: m.result === "merged" && r.phase !== "undone"; kind: "quiet"; small: true; icon: "undo"; text: "Undo"; onClicked: r.undo(r.key) }
                Btn { kind: "quiet"; small: true; text: "Open"; icon: "open"; onClicked: r.openThread(r.x_.from) }
            }
        }
    }
    Component {
        id: messageC
        Card {
            icon: "message"
            lead: "Message from"
            who: r.x_.title || "another thread"
            team: r.teamOfThread(r.x_.from)
            statusText: ({ steered: "Joined its turn", queued: "Waiting for its turn", started: "Started a turn" })[r.phase] || r.phase
            Text { width: parent.width; text: r.text; color: theme.text2; wrapMode: Text.Wrap; lineHeight: 1.2 }
        }
    }
    Component {
        id: noticeC
        Text { text: r.text; color: theme.text3; wrapMode: Text.Wrap; font.pixelSize: theme.small; leftPadding: 2 }
    }
    Component {
        id: failedC
        RowLayout {
            spacing: 8
            Icon { name: "close"; size: 14; stroke: 2.2; color: theme.red; Layout.alignment: Qt.AlignTop; Layout.topMargin: 2 }
            Text { Layout.fillWidth: true; text: r.text; color: theme.red; wrapMode: Text.Wrap }
        }
    }

    // A card (docs: Design 01): a direction icon, "Handed to Codex · work", the status on the
    // right, then the body.
    component Card: Rectangle {
        id: card
        default property alias content: inner.data
        property color accent: theme.line
        property string icon
        property color iconColor: theme.purple
        property string lead
        property string who: team ? theme.who(team) : ""
        property var team: null
        property string status: ""
        property string statusText: ""
        property color statusColor: theme.text3
        implicitHeight: inner.implicitHeight + 24
        radius: theme.radius
        color: theme.card
        border.color: accent
        Column {
            id: inner
            x: 14; y: 12
            width: parent.width - 28
            spacing: 8
            RowLayout {
                width: parent.width
                spacing: 6
                Icon { name: card.icon; size: 14; stroke: 1.9; color: card.iconColor }
                Text { text: card.lead; color: theme.text3 }
                TeamBadge { visible: card.team !== null && card.team.cli !== ""; team: card.team; size: 16; Layout.leftMargin: 2; Layout.rightMargin: 4 }
                Text { text: card.who; color: theme.text; font.weight: Font.DemiBold; elide: Text.ElideRight; Layout.fillWidth: true }
                StatusMark { visible: card.status !== ""; size: 13; state_: card.status }
                Text { visible: card.statusText !== ""; text: card.statusText; color: card.statusColor; font.pixelSize: theme.tiny }
            }
        }
    }
    // The dark box inside a card that shows what's asked (docs: Design 01, the approval).
    component Well: Rectangle {
        id: well
        default property alias content: wellBody.data
        property string why
        property string code
        width: parent.width
        implicitHeight: wellBody.implicitHeight + 20
        radius: 8
        color: theme.well
        Column {
            id: wellBody
            x: 12; y: 10
            width: parent.width - 24
            spacing: 8
            Text { text: well.why; color: theme.text3; font.pixelSize: theme.tiny }
            Text { width: parent.width; text: well.code; color: theme.text; font.family: r.mono; font.pixelSize: theme.small; wrapMode: Text.WrapAnywhere }
        }
    }
    component Footer: RowLayout {
        width: parent.width
        spacing: 6
    }
    component Chip: Rectangle {
        id: chip
        property string text
        property string icon: ""
        property bool mono: false
        property real maxWidth: 360
        implicitWidth: Math.min(chipRow.implicitWidth + 16, chip.maxWidth)
        implicitHeight: 22
        radius: theme.radiusSmall
        color: theme.selection
        clip: true
        Row {
            id: chipRow
            x: 8
            anchors.verticalCenter: parent.verticalCenter
            spacing: 5
            Icon { visible: chip.icon !== ""; anchors.verticalCenter: parent.verticalCenter; name: chip.icon; size: 12; color: theme.text3 }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                width: Math.min(implicitWidth, chip.maxWidth - 16 - (chip.icon !== "" ? 17 : 0))
                elide: Text.ElideMiddle
                text: chip.text
                color: theme.text2
                font.pixelSize: theme.tiny
                font.family: chip.mono ? r.mono : Qt.application.font.family
            }
        }
    }
}
