import QtQuick

// The design's tokens (docs: Design/README "Style used throughout"), its icons, and the CLIs'
// marks. Every screen takes its colours, sizes and drawings from here.
QtObject {
    readonly property color app: "#0D0D0D"
    readonly property color panel: "#181818"
    readonly property color card: "#1D1D1D"
    readonly property color raised: "#1F1F1F"
    readonly property color field: "#222222"
    readonly property color selection: "#262626"
    readonly property color well: "#141414"
    readonly property color line: "#2A2A2A"
    readonly property color track: "#3A3A3A"
    readonly property color text: "#ECECEC"
    readonly property color text2: "#A8A8A8"
    readonly property color text3: "#8C8C8C"
    readonly property color onLight: "#111111"
    // Colour only where it means something.
    readonly property color blue: "#6B9BFF"    // selected or on
    readonly property color amber: "#E8B04B"   // needs you, or a warning
    readonly property color red: "#FF8F87"     // failed
    readonly property color green: "#5BC48A"   // done or added
    readonly property color purple: "#B392F0"  // an agent
    readonly property int radius: 10
    readonly property int radiusSmall: 6
    readonly property int gap: 6
    readonly property int pad: 14
    readonly property int size: 13
    readonly property int small: 12
    readonly property int tiny: 11
    readonly property int row: 30

    // The design's line icons, on a 24-unit grid (docs: Design/source).
    readonly property var icons: ({
        plus: "M12 5v14M5 12h14",
        close: "M6 6l12 12M18 6L6 18",
        search: "M11 4.5a6.5 6.5 0 1 0 0 13a6.5 6.5 0 1 0 0-13zM20 20l-4.3-4.3",
        inbox: "M3 13h5l1.5 3h5l1.5-3h5M5.5 5h13l2.5 8v6H3v-6z",
        folder: "M3 6.5h6l2 2h10V19H3z",
        file: "M6.5 3h8l4 4v14h-12zM14.5 3v4h4",
        read: "M6.5 3h8l4 4v14h-12zM14.5 3v4h4M9 12h6M9 16h6",
        edit: "M4 20h4L19 9l-4-4L4 16zM14 6l4 4",
        image: "M4 5h16v14H4zM4 16l5-5 4 4 3-3 4 4M15.5 9a1 1 0 1 0 0.01 0z",
        video: "M4 6h12v12H4zM16 10l4-2.5v9L16 14",
        terminal: "M4 5h16v14H4zM7.5 9.5l3 2.5-3 2.5M12.5 15h4",
        shell: "M5 7l5 5-5 5M12 17h7",
        web: "M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18zM3 12h18M12 3c2.5 2.6 3.8 5.6 3.8 9s-1.3 6.4-3.8 9M12 3c-2.5 2.6-3.8 5.6-3.8 9s1.3 6.4 3.8 9",
        mcp: "M9 3v5M15 3v5M6 8h12v3a6 6 0 0 1-12 0zM12 17v4",
        message: "M4 5h16v11H9l-5 4z",
        people: "M9 11a3.5 3.5 0 1 0 0-7a3.5 3.5 0 1 0 0 7zM2.5 20c0-3.6 2.9-6 6.5-6s6.5 2.4 6.5 6M16 4.5a3.5 3.5 0 0 1 0 6.5M18 14.3c2 .8 3.5 2.7 3.5 5.7",
        tools: "M14.5 6.5a4 4 0 0 0-5.3 5.3L4 17l3 3 5.2-5.2a4 4 0 0 0 5.3-5.3l-2.5 2.5-2.5-2.5z",
        session: "M3.5 5h17v14h-17zM14.5 5v14",
        sidebar: "M3.5 5h17v14h-17zM9.5 5v14",
        host: "M3.5 4.5h17v6h-17zM3.5 13.5h17v6h-17zM7 7.5h.01M7 16.5h.01",
        usage: "M5 20v-8M12 20V5M19 20v-5",
        settings: "M12 9a3 3 0 1 0 0 6a3 3 0 1 0 0-6zM12 2.5v3M12 18.5v3M2.5 12h3M18.5 12h3M5.3 5.3l2.1 2.1M16.6 16.6l2.1 2.1M5.3 18.7l2.1-2.1M16.6 7.4l2.1-2.1",
        handOut: "M7 17L17 7M9 7h8v8",
        handIn: "M17 7L7 17M15 17H7V9",
        open: "M10 7l5 5-5 5",
        back: "M14 6l-6 6 6 6",
        down: "M7 10l5 5 5-5",
        up: "M7 14l5-5 5 5",
        upDown: "M8 9l4-4 4 4M8 15l4 4 4-4",
        send: "M12 19V6M6 12l6-6 6 6",
        undo: "M5 12a7 7 0 1 0 2.05-4.95M5 5v4h4",
        external: "M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5",
        stop: "M7 7h10v10H7z",
        pause: "M8.5 5v14M15.5 5v14",
        check: "M5 12.5l4.5 4.5L19 7.5",
        approvals: "M5 12.5l4.5 4.5L19 7.5M3 12a9 9 0 1 0 18 0a9 9 0 1 0-18 0",
        ring: "M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18z",
        arc: "M12 3a9 9 0 0 1 9 9",
        dot: "M12 7.5a4.5 4.5 0 1 0 0 9a4.5 4.5 0 1 0 0-9z",
        warning: "M12 3.5l9.5 16.5h-19zM12 10v4.5M12 17.5v.01",
        branch: "M6 3v12M18 9a3 3 0 1 0 0-6a3 3 0 1 0 0 6zM6 21a3 3 0 1 0 0-6a3 3 0 1 0 0 6zM18 9c0 6-12 3-12 9"
    })

    // Each CLI's mark and name (docs: Design 01, the team icons).
    readonly property var clis: ({
        claude: { label: "Claude", glyph: "M12 4v16M4 12h16M6.3 6.3l11.4 11.4M17.7 6.3L6.3 17.7" },
        codex: { label: "Codex", glyph: "M9 7l-5 5 5 5M15 7l5 5-5 5" },
        agy: { label: "Antigravity", glyph: "M12 20V5M6 11l6-6 6 6" }
    })
    function cli(name) { return clis[name] || { label: name || "?", glyph: icons.tools } }
    // A team's two letters: its first two words' initials, or its first two letters.
    function initials(name) {
        const words = (name || "").split(/[\s_\-.]+/).filter(w => w !== "")
        const s = words.length > 1 ? words[0][0] + words[1][0] : (words[0] || "?").slice(0, 2)
        return s.toUpperCase()
    }
    // "Codex · work": a team as the design names it.
    function who(team) { return team ? cli(team.cli).label + " · " + team.name : "no team" }
    // How long ago, briefly: "now", "4 min", "2 h", "3 d".
    function ago(ms) {
        if (!ms) return ""
        const m = Math.max(0, Math.round((Date.now() - ms) / 60000))
        return m < 1 ? "now" : m < 60 ? m + " min" : m < 1440 ? Math.round(m / 60) + " h" : Math.round(m / 1440) + " d"
    }
    // A freedom level as the picker names it.
    function freedomLabel(id) {
        return ({ plan: "Plan only", ask: "Ask me", sandbox: "Sandbox free", full: "Full auto" })[id] || id
    }
}
