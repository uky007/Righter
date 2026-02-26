use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

use crate::editor::Editor;
use crate::input::mode::Mode;

pub struct StatusLine<'a> {
    editor: &'a Editor,
}

impl<'a> StatusLine<'a> {
    pub fn new(editor: &'a Editor) -> Self {
        Self { editor }
    }
}

impl Widget for StatusLine<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let (mode_str, mode_style) = match self.editor.mode {
            Mode::Normal => (
                " NORMAL ",
                Style::default()
                    .bg(Color::Blue)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ),
            Mode::Insert => (
                " INSERT ",
                Style::default()
                    .bg(Color::Green)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ),
            Mode::Visual => (
                " VISUAL ",
                Style::default()
                    .bg(Color::Magenta)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ),
            Mode::VisualLine => (
                " V-LINE ",
                Style::default()
                    .bg(Color::Magenta)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ),
            Mode::Command => (
                " COMMAND ",
                Style::default()
                    .bg(Color::Yellow)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ),
            Mode::Search => (
                " SEARCH ",
                Style::default()
                    .bg(Color::Cyan)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ),
        };

        let file_name = self.editor.document.file_name();
        let modified = if self.editor.document.modified {
            " [+]"
        } else {
            ""
        };
        let file_info = format!(" {file_name}{modified}");

        // Diagnostic counts
        let errors = self
            .editor
            .diagnostics
            .iter()
            .filter(|d| d.severity == 1)
            .count();
        let warnings = self
            .editor
            .diagnostics
            .iter()
            .filter(|d| d.severity == 2)
            .count();
        let diag_str = if errors > 0 || warnings > 0 {
            format!(" E:{errors} W:{warnings}")
        } else {
            String::new()
        };

        let position = format!(
            "{} {}:{} ",
            diag_str,
            self.editor.cursor.row + 1,
            self.editor.cursor.col + 1,
        );

        let bar_style = Style::default().bg(Color::DarkGray).fg(Color::White);

        // Fill background
        for x in 0..area.width {
            buf[(area.x + x, area.y)]
                .set_char(' ')
                .set_style(bar_style);
        }

        // Mode indicator
        let mut x = area.x;
        for ch in mode_str.chars() {
            if x < area.right() {
                buf[(x, area.y)].set_char(ch).set_style(mode_style);
                x += 1;
            }
        }

        // File info
        for ch in file_info.chars() {
            if x < area.right() {
                buf[(x, area.y)].set_char(ch).set_style(bar_style);
                x += 1;
            }
        }

        // Position (right-aligned)
        let pos_start = area.right().saturating_sub(position.len() as u16);
        let mut px = pos_start;
        for ch in position.chars() {
            if px < area.right() {
                buf[(px, area.y)].set_char(ch).set_style(bar_style);
                px += 1;
            }
        }
    }
}
