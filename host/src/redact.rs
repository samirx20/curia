//! Removing secrets before anything is stored or sent (CARRY_OVER §21): Bearer and Basic
//! tokens, `token=` / `api_key=` / `password=` / `secret=` values, `sk-…` keys, credentials in
//! URLs, and control characters.
use curia_core::{Event, TurnOutcome};

const HIDDEN: &str = "[redacted]";
/// Longest notice or error message kept.
const MAX_MESSAGE: usize = 4096;

/// Redacts the free text in an event that a CLI or a tool produced.
pub fn event(e: &mut Event) {
    match e {
        Event::ToolResult { output, .. } => *output = redact(output),
        Event::Notice { text } => *text = cap(redact(text)),
        Event::TurnFinished { outcome: TurnOutcome::Failed { message }, .. } => *message = cap(redact(message)),
        _ => {}
    }
}

fn cap(mut s: String) -> String {
    if s.len() > MAX_MESSAGE {
        let mut cut = MAX_MESSAGE;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        s.truncate(cut);
        s.push('…');
    }
    s
}

pub fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let words: Vec<&str> = text.split_inclusive(|c: char| c.is_whitespace()).collect();
    let mut hide_next = false;
    for w in words {
        let (word, space) = w.split_at(w.trim_end().len());
        let lower = word.to_ascii_lowercase();
        let redacted = if hide_next && !word.is_empty() {
            hide_next = false;
            HIDDEN.to_string()
        } else if lower == "bearer" || lower == "basic" {
            hide_next = true;
            word.to_string()
        } else {
            redact_word(word)
        };
        out.push_str(&redacted);
        out.push_str(space);
    }
    out.chars().filter(|c| !c.is_control() || matches!(c, '\n' | '\t' | '\r')).collect()
}

fn redact_word(word: &str) -> String {
    let lower = word.to_ascii_lowercase();
    for key in ["token", "api_key", "apikey", "password", "passwd", "secret", "access_key"] {
        if let Some(at) = lower.find(&format!("{key}=")).or_else(|| lower.find(&format!("{key}\":\""))) {
            let start = at + key.len() + if lower[at + key.len()..].starts_with("=") { 1 } else { 3 };
            let end = word[start..].find(['&', '"', '\'', ',', ';']).map_or(word.len(), |i| start + i);
            if end > start {
                return format!("{}{HIDDEN}{}", &word[..start], &word[end..]);
            }
        }
    }
    if let Some(at) = word.find("sk-") {
        let rest = &word[at + 3..];
        let len = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').count();
        if len >= 16 {
            return format!("{}{HIDDEN}{}", &word[..at], &rest[len..]);
        }
    }
    if let Some(scheme) = word.find("://") {
        let host_start = scheme + 3;
        if let Some(at) = word[host_start..].find('@') {
            let creds = &word[host_start..host_start + at];
            if creds.contains(':') && !creds.contains('/') {
                return format!("{}{HIDDEN}{}", &word[..host_start], &word[host_start + at..]);
            }
        }
    }
    word.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_hidden_and_ordinary_text_is_kept() {
        assert_eq!(redact("Authorization: Bearer abc.def.ghi"), "Authorization: Bearer [redacted]");
        assert_eq!(redact("url?token=s3cr3t&x=1"), "url?token=[redacted]&x=1");
        assert_eq!(redact("export OPENAI=sk-abcdefghijklmnopqrstuvwx"), "export OPENAI=[redacted]");
        assert_eq!(redact("git clone https://sam:hunter2@github.com/x"), "git clone https://[redacted]@github.com/x");
        assert_eq!(redact("{\"password\":\"hunter2\",\"user\":\"sam\"}"), "{\"password\":\"[redacted]\",\"user\":\"sam\"}");
        assert_eq!(redact("an ordinary line\nwith sk-short and a@b"), "an ordinary line\nwith sk-short and a@b");
        assert_eq!(redact("bell\u{7}gone"), "bellgone");
    }

    #[test]
    fn long_messages_are_capped() {
        let mut e = Event::Notice { text: "x".repeat(5000) };
        event(&mut e);
        let Event::Notice { text } = e else { unreachable!() };
        assert!(text.len() <= MAX_MESSAGE + 3);
    }
}
