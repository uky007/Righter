use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

use crate::buffer;
use crate::editor::pane::PaneRenderData;
use crate::editor::selection::Position;
use crate::editor::wrap;
use crate::input::mode::Mode;

pub struct EditorView<'a> {
    data: PaneRenderData<'a>,
}

impl<'a> EditorView<'a> {
    pub fn new(data: PaneRenderData<'a>) -> Self {
        Self { data }
    }

    fn gutter_width(&self) -> u16 {
        let lines = self.data.document.line_count();
        let digits = if lines == 0 {
            1
        } else {
            (lines as f64).log10().floor() as u16 + 1
        };
        digits + 2
    }

    fn is_selected(&self, row: usize, col: usize) -> bool {
        if let Some((start, end)) = self.data.selection_range() {
            let pos = Position { row, col };
            pos >= start && pos <= end
        } else {
            false
        }
    }

    fn diagnostic_severity_at(&self, row: usize) -> Option<u8> {
        let mut worst: Option<u8> = None;
        for d in self.data.diagnostics {
            if d.start_line as usize <= row && row <= d.end_line as usize {
                let sev = d.severity;
                worst = Some(match worst {
                    Some(w) => w.min(sev),
                    None => sev,
                });
            }
        }
        worst
    }

    fn char_style(&self, doc_row: usize, char_idx: usize) -> Style {
        let is_cursor = self.data.is_active
            && doc_row == self.data.cursor.row
            && char_idx == self.data.cursor.col
            && !matches!(self.data.mode, Mode::Insert);

        let is_selected = self.is_selected(doc_row, char_idx);

        // Base style from syntax highlighting
        let hl = self.data.highlight_style_at(doc_row, char_idx);

        // Apply diagnostic line background to base style
        let diag_sev = self.diagnostic_severity_at(doc_row);
        let hl = match diag_sev {
            Some(1) => hl.bg(Color::Rgb(50, 20, 20)),
            Some(2) => hl.bg(Color::Rgb(45, 40, 20)),
            _ => hl,
        };

        if is_cursor {
            Style::default()
                .bg(Color::White)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD)
        } else if is_selected {
            hl.bg(Color::LightBlue)
        } else {
            // Search match highlighting
            let is_search_match = self.data.is_search_match(doc_row, char_idx);

            // Underline diagnostic ranges (the specific token, not whole line)
            let in_diag = self.data.diagnostics.iter().any(|d| {
                let start_row = d.start_line as usize;
                let end_row = d.end_line as usize;
                let start_col = d.start_col as usize;
                let end_col = d.end_col as usize;
                if doc_row < start_row || doc_row > end_row {
                    return false;
                }
                if doc_row == start_row && char_idx < start_col {
                    return false;
                }
                if doc_row == end_row && char_idx >= end_col {
                    return false;
                }
                true
            });

            // Bracket matching highlight
            let is_matching_bracket = self
                .data
                .matching_bracket
                .is_some_and(|pos| pos.row == doc_row && pos.col == char_idx);

            if is_search_match {
                Style::default()
                    .bg(Color::Rgb(180, 150, 50))
                    .fg(Color::Black)
            } else if is_matching_bracket {
                hl.bg(Color::Rgb(60, 65, 80))
                    .add_modifier(Modifier::BOLD)
            } else if in_diag {
                hl.add_modifier(Modifier::UNDERLINED)
            } else {
                hl
            }
        }
    }

    /// Draw the gutter (line number + diagnostic sign) for a given doc_row.
    fn draw_gutter(&self, area: Rect, buf: &mut Buffer, screen_y: u16, doc_row: usize, gutter_w: u16, is_first_segment: bool) {
        if !is_first_segment {
            // Continuation lines: blank gutter
            for x in 0..gutter_w {
                if area.x + x < area.right() {
                    buf[(area.x + x, screen_y)]
                        .set_char(' ')
                        .set_style(Style::default().fg(Color::DarkGray));
                }
            }
            return;
        }

        let diag_sev = self.diagnostic_severity_at(doc_row);
        let line_num = format!("{:>width$} ", doc_row + 1, width = (gutter_w - 1) as usize);
        let is_cursor_line = self.data.is_active && doc_row == self.data.cursor.row;
        let gutter_style = match diag_sev {
            Some(1) => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            Some(2) => Style::default().fg(Color::Yellow),
            _ if is_cursor_line => Style::default().fg(Color::Yellow),
            _ => Style::default().fg(Color::DarkGray),
        };
        for (x, ch) in line_num.chars().enumerate() {
            if x < gutter_w as usize && area.x + x as u16 <= area.right() {
                buf[(area.x + x as u16, screen_y)]
                    .set_char(ch)
                    .set_style(gutter_style);
            }
        }
        // Overlay diagnostic sign
        if let Some(sev) = diag_sev {
            let (sign, color) = match sev {
                1 => ('●', Color::Red),
                2 => ('▲', Color::Yellow),
                3 => ('■', Color::Blue),
                _ => ('·', Color::Cyan),
            };
            if area.x < area.right() {
                buf[(area.x, screen_y)]
                    .set_char(sign)
                    .set_style(Style::default().fg(color).add_modifier(Modifier::BOLD));
            }
        }
    }

    /// Fill a screen line with diagnostic background tint.
    fn fill_diag_bg(&self, area: Rect, buf: &mut Buffer, screen_y: u16, doc_row: usize, gutter_w: u16) {
        let diag_sev = self.diagnostic_severity_at(doc_row);
        let diag_bg = match diag_sev {
            Some(1) => Some(Color::Rgb(50, 20, 20)),
            Some(2) => Some(Color::Rgb(45, 40, 20)),
            _ => None,
        };
        if let Some(bg) = diag_bg {
            for dx in gutter_w..area.width {
                let x = area.x + dx;
                if x < area.right() {
                    buf[(x, screen_y)]
                        .set_char(' ')
                        .set_style(Style::default().bg(bg));
                }
            }
        }
    }

    /// Render without wrapping (original logic).
    fn render_nowrap(self, area: Rect, buf: &mut Buffer, gutter_w: u16, text_width: u16) {
        let offset_row = self.data.view.offset_row;
        let offset_col = self.data.view.offset_col;

        for y in 0..area.height {
            let doc_row = offset_row + y as usize;
            let screen_y = area.y + y;

            if doc_row < self.data.document.line_count() {
                self.draw_gutter(area, buf, screen_y, doc_row, gutter_w, true);
                self.fill_diag_bg(area, buf, screen_y, doc_row, gutter_w);

                // Draw text
                let line = self.data.document.rope.line(doc_row);
                let line_len = buffer::line_display_len(line);
                let mut text_x: u16 = 0;

                for char_idx in offset_col..line_len {
                    if text_x >= text_width {
                        break;
                    }
                    let ch = line.char(char_idx);
                    let display_ch = if ch == '\t' { ' ' } else { ch };
                    let w = unicode_width::UnicodeWidthChar::width(display_ch).unwrap_or(1);

                    let screen_x = area.x + gutter_w + text_x;
                    if screen_x < area.right() {
                        let style = self.char_style(doc_row, char_idx);
                        buf[(screen_x, screen_y)]
                            .set_char(display_ch)
                            .set_style(style);
                    }
                    text_x += w as u16;
                }

                // Draw cursor at end of line in insert mode
                if self.data.is_active
                    && self.data.mode == Mode::Insert
                    && doc_row == self.data.cursor.row
                    && self.data.cursor.col >= line_len
                {
                    let cursor_x =
                        area.x + gutter_w + (self.data.cursor.col - offset_col) as u16;
                    if cursor_x < area.right() {
                        buf[(cursor_x, screen_y)].set_char(' ').set_style(
                            Style::default().bg(Color::White).fg(Color::Black),
                        );
                    }
                }

                // In non-insert mode, if cursor is on an empty line
                if self.data.is_active
                    && !matches!(self.data.mode, Mode::Insert)
                    && doc_row == self.data.cursor.row
                    && line_len == 0
                {
                    let cursor_x = area.x + gutter_w;
                    if cursor_x < area.right() {
                        buf[(cursor_x, screen_y)].set_char(' ').set_style(
                            Style::default().bg(Color::White).fg(Color::Black),
                        );
                    }
                }

                // In visual line mode, highlight remaining space on selected lines
                if self.data.mode == Mode::VisualLine && self.is_selected(doc_row, 0) {
                    let start_x = area.x + gutter_w + text_x;
                    let sel_style = Style::default().bg(Color::LightBlue).fg(Color::Black);
                    for sx in start_x..area.right() {
                        buf[(sx, screen_y)].set_style(sel_style);
                    }
                }
            } else {
                // Draw tilde for lines past end of document
                let tilde_x = area.x + gutter_w.saturating_sub(2);
                if tilde_x < area.right() {
                    buf[(tilde_x, screen_y)]
                        .set_char('~')
                        .set_style(Style::default().fg(Color::DarkGray));
                }
            }
        }
    }

    /// Render with line wrapping enabled.
    fn render_wrapped(self, area: Rect, buf: &mut Buffer, gutter_w: u16, text_width: u16) {
        let screen_map = wrap::build_screen_map(
            &self.data.document.rope,
            self.data.view.offset_row,
            self.data.view.offset_wrap,
            text_width,
            area.height,
        );

        for (y, seg) in screen_map.iter().enumerate() {
            let screen_y = area.y + y as u16;
            let doc_row = seg.doc_row;
            let is_first_segment = seg.segment_index == 0;

            self.draw_gutter(area, buf, screen_y, doc_row, gutter_w, is_first_segment);
            self.fill_diag_bg(area, buf, screen_y, doc_row, gutter_w);

            // Draw text for this segment
            let line = self.data.document.rope.line(doc_row);
            let line_len = buffer::line_display_len(line);
            let mut text_x: u16 = 0;

            for char_idx in seg.char_start..seg.char_end.min(line_len) {
                if text_x >= text_width {
                    break;
                }
                let ch = line.char(char_idx);
                let display_ch = if ch == '\t' { ' ' } else { ch };
                let w = unicode_width::UnicodeWidthChar::width(display_ch).unwrap_or(1);

                let screen_x = area.x + gutter_w + text_x;
                if screen_x < area.right() {
                    let style = self.char_style(doc_row, char_idx);
                    buf[(screen_x, screen_y)]
                        .set_char(display_ch)
                        .set_style(style);
                }
                text_x += w as u16;
            }

            // Draw cursor at end of line in insert mode
            if self.data.is_active
                && self.data.mode == Mode::Insert
                && doc_row == self.data.cursor.row
                && self.data.cursor.col >= line_len
                && is_first_segment
                && seg.char_end >= line_len
            {
                // Cursor past end of line — show on this segment if it's the last one
                let cursor_display_col = {
                    // Calculate display position of cursor.col within this segment
                    let mut dx: u16 = 0;
                    for ci in seg.char_start..self.data.cursor.col.min(line_len) {
                        let c = line.char(ci);
                        let dc = if c == '\t' { ' ' } else { c };
                        dx += unicode_width::UnicodeWidthChar::width(dc).unwrap_or(1) as u16;
                    }
                    dx
                };
                let cursor_x = area.x + gutter_w + cursor_display_col;
                if cursor_x < area.right() {
                    buf[(cursor_x, screen_y)].set_char(' ').set_style(
                        Style::default().bg(Color::White).fg(Color::Black),
                    );
                }
            } else if self.data.is_active
                && self.data.mode == Mode::Insert
                && doc_row == self.data.cursor.row
                && self.data.cursor.col >= seg.char_start
                && self.data.cursor.col >= line_len
                && seg.char_end >= line_len
            {
                // Insert cursor past end on last segment
                let cursor_x = area.x + gutter_w + text_x;
                if cursor_x < area.right() {
                    buf[(cursor_x, screen_y)].set_char(' ').set_style(
                        Style::default().bg(Color::White).fg(Color::Black),
                    );
                }
            }

            // In non-insert mode, if cursor is on an empty line
            if self.data.is_active
                && !matches!(self.data.mode, Mode::Insert)
                && doc_row == self.data.cursor.row
                && line_len == 0
                && is_first_segment
            {
                let cursor_x = area.x + gutter_w;
                if cursor_x < area.right() {
                    buf[(cursor_x, screen_y)].set_char(' ').set_style(
                        Style::default().bg(Color::White).fg(Color::Black),
                    );
                }
            }

            // In visual line mode, highlight remaining space on selected lines
            if self.data.mode == Mode::VisualLine && self.is_selected(doc_row, 0) {
                let start_x = area.x + gutter_w + text_x;
                let sel_style = Style::default().bg(Color::LightBlue).fg(Color::Black);
                for sx in start_x..area.right() {
                    buf[(sx, screen_y)].set_style(sel_style);
                }
            }
        }

        // Fill remaining screen lines with tildes
        for y in screen_map.len()..area.height as usize {
            let screen_y = area.y + y as u16;
            let tilde_x = area.x + gutter_w.saturating_sub(2);
            if tilde_x < area.right() {
                buf[(tilde_x, screen_y)]
                    .set_char('~')
                    .set_style(Style::default().fg(Color::DarkGray));
            }
        }
    }
}

impl Widget for EditorView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let gutter_w = self.gutter_width();
        let text_width = area.width.saturating_sub(gutter_w);

        if self.data.view.wrap && text_width > 0 {
            self.render_wrapped(area, buf, gutter_w, text_width);
        } else {
            self.render_nowrap(area, buf, gutter_w, text_width);
        }
    }
}
