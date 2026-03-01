/// Frontend-independent syntax style.
/// Converted to ratatui::style::Style (TUI) or egui text color (GUI) at render time.
#[derive(Debug, Clone, Copy, Default)]
pub struct SyntaxStyle {
    pub fg: Option<RgbColor>,
    pub italic: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct RgbColor(pub u8, pub u8, pub u8);

impl SyntaxStyle {
    pub fn fg(mut self, color: RgbColor) -> Self {
        self.fg = Some(color);
        self
    }

    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }
}
