//! A terminal's screen as clients get it: rows of styled runs, and the cursor. The host keeps
//! the real terminal (alacritty_terminal); clients only draw this, so every client shows the
//! same screen and none needs a terminal emulator of its own.
use alacritty_terminal::Term;
use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::{Color, NamedColor};
use serde::Serialize;

/// The size a terminal is made with (alacritty asks for it as `Dimensions`).
#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub cols: u16,
    pub rows: u16,
}

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.rows as usize
    }
    fn screen_lines(&self) -> usize {
        self.rows as usize
    }
    fn columns(&self) -> usize {
        self.cols as usize
    }
}

/// A stretch of text in one style. Colours are `#rrggbb`; none means the default.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Run {
    pub t: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub b: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub i: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub u: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Screen {
    pub cols: u16,
    pub rows: u16,
    pub cursor: (u16, u16),
    pub lines: Vec<Vec<Run>>,
}

/// The 16 ANSI colours, tuned to sit on the app's dark panels.
const PALETTE: [&str; 16] = [
    "#1f1f1f", "#ff8f87", "#5bc48a", "#e8b04b", "#6b9bff", "#b392f0", "#5ec4c8", "#c8c8c8",
    "#6a6a6a", "#ffaaa3", "#7ddca6", "#f2c66d", "#8fb4ff", "#c9adf5", "#86dde0", "#ececec",
];

fn indexed(n: u8) -> String {
    match n {
        0..=15 => PALETTE[n as usize].to_string(),
        16..=231 => {
            let n = n - 16;
            let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            format!("#{:02x}{:02x}{:02x}", level(n / 36), level((n / 6) % 6), level(n % 6))
        }
        _ => {
            let g = 8 + (n - 232) * 10;
            format!("#{g:02x}{g:02x}{g:02x}")
        }
    }
}

fn color(c: Color) -> Option<String> {
    match c {
        Color::Spec(rgb) => Some(format!("#{:02x}{:02x}{:02x}", rgb.r, rgb.g, rgb.b)),
        Color::Indexed(n) => Some(indexed(n)),
        Color::Named(n) => {
            let i = n as usize;
            if i < 16 {
                Some(PALETTE[i].to_string())
            } else if matches!(n, NamedColor::DimForeground | NamedColor::BrightForeground) {
                Some(PALETTE[if n == NamedColor::DimForeground { 8 } else { 15 }].to_string())
            } else {
                None // the default foreground or background
            }
        }
    }
}

/// Reads the visible screen out of the terminal.
pub fn snapshot<L: EventListener>(term: &Term<L>, size: Size) -> Screen {
    let content = term.renderable_content();
    let cursor = content.cursor.point;
    let mut lines: Vec<Vec<Run>> = vec![Vec::new(); size.rows as usize];
    for cell in content.display_iter {
        let row = cell.point.line.0 + content.display_offset as i32;
        if row < 0 || row as usize >= lines.len() || cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
            continue;
        }
        let (mut fg, mut bg) = (color(cell.fg), color(cell.bg));
        if cell.flags.contains(Flags::INVERSE) {
            (fg, bg) = (bg.or(Some("#0d0d0d".into())), fg.or(Some("#ececec".into())));
        }
        let run = Run {
            t: cell.c.to_string(),
            fg,
            bg,
            b: cell.flags.contains(Flags::BOLD),
            i: cell.flags.contains(Flags::ITALIC),
            u: cell.flags.contains(Flags::UNDERLINE),
        };
        let line = &mut lines[row as usize];
        match line.last_mut() {
            Some(last) if (&last.fg, &last.bg, last.b, last.i, last.u) == (&run.fg, &run.bg, run.b, run.i, run.u) => last.t.push_str(&run.t),
            _ => line.push(run),
        }
    }
    // Trailing blanks in the default style carry nothing.
    for line in &mut lines {
        if let Some(last) = line.last_mut() {
            if last.bg.is_none() {
                last.t.truncate(last.t.trim_end().len());
                if last.t.is_empty() {
                    line.pop();
                }
            }
        }
    }
    let row = (cursor.line.0 + content.display_offset as i32).max(0) as u16;
    Screen { cols: size.cols, rows: size.rows, cursor: (row, cursor.column.0 as u16), lines }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::event::VoidListener;
    use alacritty_terminal::term::Config;
    use alacritty_terminal::vte::ansi::Processor;

    #[test]
    fn coloured_output_becomes_styled_runs() {
        let size = Size { cols: 20, rows: 3 };
        let mut term = Term::new(Config::default(), &size, VoidListener);
        let mut parser: Processor = Processor::new();
        parser.advance(&mut term, b"ok \x1b[32mpass\x1b[0m\r\n\x1b[1;31mFAILED\x1b[0m");
        let s = snapshot(&term, size);
        assert_eq!(s.lines[0][0].t, "ok ");
        assert_eq!((s.lines[0][1].t.as_str(), s.lines[0][1].fg.as_deref()), ("pass", Some("#5bc48a")));
        assert!(s.lines[1][0].b && s.lines[1][0].fg.as_deref() == Some("#ff8f87"));
        assert_eq!(s.cursor, (1, 6));
        assert!(s.lines[2].is_empty());
    }

    #[test]
    fn the_256_colour_cube_and_greys_map_to_rgb() {
        assert_eq!(indexed(16), "#000000");
        assert_eq!(indexed(231), "#ffffff");
        assert_eq!(indexed(232), "#080808");
    }
}
