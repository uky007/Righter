#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
    VisualLine,
    Command,
    Search,
}

impl Mode {
    pub fn is_visual(&self) -> bool {
        matches!(self, Mode::Visual | Mode::VisualLine)
    }
}

impl Default for Mode {
    fn default() -> Self {
        Mode::Normal
    }
}
