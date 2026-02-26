use crate::editor::selection::Position;

#[derive(Debug, Default, Clone, Copy)]
pub struct View {
    pub offset_row: usize,
    pub offset_col: usize,
    pub width: u16,
    pub height: u16,
}

impl View {
    pub fn ensure_cursor_visible(&mut self, cursor: &Position, scroll_off: usize) {
        let scrolloff = scroll_off.min(self.height as usize / 2);

        // Vertical scrolling
        if cursor.row < self.offset_row + scrolloff {
            self.offset_row = cursor.row.saturating_sub(scrolloff);
        }
        if cursor.row >= self.offset_row + self.height as usize - scrolloff {
            self.offset_row = cursor
                .row
                .saturating_sub(self.height as usize - 1 - scrolloff);
        }

        // Horizontal scrolling
        if cursor.col < self.offset_col {
            self.offset_col = cursor.col;
        }
        if cursor.col >= self.offset_col + self.width as usize {
            self.offset_col = cursor.col.saturating_sub(self.width as usize - 1);
        }
    }
}
