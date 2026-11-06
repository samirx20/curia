import QtQuick
import QtQuick.Layouts

// Settings > Hand-offs and routing (docs: Design 16): what happens when a team runs out, and
// what the host does after a limit resets or a restart.
ColumnLayout {
    id: routing
    property var page
    readonly property var s: page ? page.win.settings : ({})
    spacing: 4

    SettingCard {
        title: "When a team runs out"
        SettingRow {
            first: true
            title: "Move work to another team"
            hint: "To a team of the same CLI that can carry on the conversation. Otherwise the thread waits for the reset."
            Toggle { on: routing.s.reroute_when_out === true; onToggled: on => routing.page.setSetting("reroute_when_out", on) }
        }
        SettingRow {
            title: "Continue after the reset"
            hint: "Tell a thread its limit stopped to continue where it left off, once the limit resets."
            Toggle { on: routing.s.continue_after_reset !== false; onToggled: on => routing.page.setSetting("continue_after_reset", on) }
        }
    }
    SettingCard {
        title: "When the host restarts"
        SettingRow {
            first: true
            title: "Continue cut-off turns"
            hint: "Turns a restart cut off carry on by themselves (at most three times in a row)."
            Toggle { on: routing.s.continue_after_restart !== false; onToggled: on => routing.page.setSetting("continue_after_restart", on) }
        }
    }
}
