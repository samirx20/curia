//! Freedom levels (Scope §3.4): how much an agent may do without asking. Curia speaks in
//! levels; each CLI adapter maps them to its own modes. A level a CLI lacks falls back to a
//! more cautious one, never an imitation (CARRY_OVER §20).

/// The levels, most cautious first.
pub const LEVELS: &[&str] = &["plan", "ask", "sandbox", "full"];

/// Normalises a level, accepting the older Claude mode names too. Unknown → `ask`.
pub fn level(name: &str) -> &'static str {
    match name {
        "plan" => "plan",
        "ask" | "manual" | "default" => "ask",
        "sandbox" | "acceptEdits" => "sandbox",
        "full" | "bypassPermissions" => "full",
        _ => "ask",
    }
}

/// How much freedom a level gives (to keep a child within its parent's).
pub fn rank(name: &str) -> usize {
    let l = level(name);
    LEVELS.iter().position(|x| *x == l).unwrap_or(1)
}

/// Claude Code's `--permission-mode` for a level. "Sandbox free" would let commands inside the
/// sandbox run too; Claude's nearest mode, `acceptEdits`, still asks for commands, which is the
/// more cautious side.
pub fn claude_mode(name: &str) -> &'static str {
    match level(name) {
        "plan" => "plan",
        "sandbox" => "acceptEdits",
        "full" => "bypassPermissions",
        _ => "manual",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_map_to_claude_modes_and_unknown_is_cautious() {
        assert_eq!(claude_mode("ask"), "manual");
        assert_eq!(claude_mode("sandbox"), "acceptEdits");
        assert_eq!(claude_mode("full"), "bypassPermissions");
        assert_eq!(claude_mode("acceptEdits"), "acceptEdits", "older names still work");
        assert_eq!(claude_mode("whatever"), "manual");
        assert!(rank("full") > rank("sandbox") && rank("sandbox") > rank("ask") && rank("ask") > rank("plan"));
    }
}
