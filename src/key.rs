/// Frontend-independent key representation.
/// Converted from crossterm (TUI) or egui (GUI) key events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyInput {
    pub code: KeyCode,
    pub ctrl: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    Char(char),
    Esc,
    Enter,
    Backspace,
    Tab,
    BackTab,
    Up,
    Down,
    Left,
    Right,
}

#[cfg(feature = "tui")]
#[allow(dead_code)]
impl KeyInput {
    pub fn from_crossterm(key: crossterm::event::KeyEvent) -> Option<Self> {
        use crossterm::event::{KeyCode as CK, KeyModifiers};
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let code = match key.code {
            CK::Char(c) => KeyCode::Char(c),
            CK::Esc => KeyCode::Esc,
            CK::Enter => KeyCode::Enter,
            CK::Backspace => KeyCode::Backspace,
            CK::Tab => KeyCode::Tab,
            CK::BackTab => KeyCode::BackTab,
            CK::Up => KeyCode::Up,
            CK::Down => KeyCode::Down,
            CK::Left => KeyCode::Left,
            CK::Right => KeyCode::Right,
            _ => return None,
        };
        Some(KeyInput { code, ctrl })
    }
}
