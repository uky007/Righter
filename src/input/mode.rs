#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum Mode {
    #[default]
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

