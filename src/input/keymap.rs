use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::editor::Editor;
use crate::input::command::{CaseOp, Command, Motion};
use crate::input::mode::Mode;

pub fn map_key(editor: &mut Editor, key: KeyEvent) -> Option<Command> {
    // File finder intercepts all keys when showing
    if editor.showing_file_finder {
        return map_file_finder(key);
    }

    match editor.mode {
        Mode::Normal => map_normal(editor, key),
        Mode::Insert => map_insert(editor, key),
        Mode::Visual | Mode::VisualLine => map_visual(key),
        Mode::Command => map_command(key),
        Mode::Search => map_search(key),
    }
}

fn map_normal(editor: &mut Editor, key: KeyEvent) -> Option<Command> {
    // Dismiss popups on any key if showing hover or references
    if editor.showing_hover {
        editor.showing_hover = false;
        editor.hover_text = None;
        return None;
    }
    if editor.showing_references {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => return Some(Command::ReferenceNext),
            KeyCode::Char('k') | KeyCode::Up => return Some(Command::ReferencePrev),
            KeyCode::Enter => return Some(Command::ReferenceJump),
            KeyCode::Esc | KeyCode::Char('q') => return Some(Command::DismissPopup),
            _ => return Some(Command::DismissPopup),
        }
    }
    if editor.showing_code_actions {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => return Some(Command::CodeActionNext),
            KeyCode::Char('k') | KeyCode::Up => return Some(Command::CodeActionPrev),
            KeyCode::Enter => return Some(Command::CodeActionAccept),
            KeyCode::Esc | KeyCode::Char('q') => return Some(Command::CodeActionDismiss),
            _ => return Some(Command::CodeActionDismiss),
        }
    }
    if editor.showing_diagnostics {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => return Some(Command::DiagnosticNext),
            KeyCode::Char('k') | KeyCode::Up => return Some(Command::DiagnosticPrev),
            KeyCode::Enter => return Some(Command::DiagnosticJump),
            KeyCode::Esc | KeyCode::Char('q') => return Some(Command::DismissPopup),
            _ => return None,
        }
    }

    // Handle pending keys (operators, g-prefix, etc.)
    if !editor.pending_keys.is_empty() {
        return handle_pending(editor, key);
    }

    // Ctrl-modified keys first (before plain char matches)
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('d') => return Some(Command::HalfPageDown),
            KeyCode::Char('u') => return Some(Command::HalfPageUp),
            KeyCode::Char('f') => return Some(Command::FullPageDown),
            KeyCode::Char('b') => return Some(Command::FullPageUp),
            KeyCode::Char('r') => return Some(Command::Redo),
            KeyCode::Char('p') => return Some(Command::OpenFileFinder),
            KeyCode::Char('o') => return Some(Command::JumpBack),
            KeyCode::Char('i') => return Some(Command::JumpForward),
            KeyCode::Char('a') => return Some(Command::IncrementNumber),
            KeyCode::Char('x') => return Some(Command::DecrementNumber),
            KeyCode::Char('w') => {
                editor.pending_keys.push('W'); // uppercase to avoid collision
                return None;
            }
            KeyCode::Char('c') => {
                editor.should_quit = true;
                return None;
            }
            _ => return None,
        }
    }

    match key.code {
        // Movement
        KeyCode::Char('h') | KeyCode::Left => Some(Command::MoveLeft),
        KeyCode::Char('j') | KeyCode::Down => Some(Command::MoveDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Command::MoveUp),
        KeyCode::Char('l') | KeyCode::Right => Some(Command::MoveRight),
        KeyCode::Char('w') => Some(Command::MoveWordForward),
        KeyCode::Char('b') => Some(Command::MoveWordBackward),
        KeyCode::Char('e') => Some(Command::MoveWordEnd),
        KeyCode::Char('0') => Some(Command::MoveLineStart),
        KeyCode::Char('$') => Some(Command::MoveLineEnd),
        KeyCode::Char('^') => Some(Command::MoveFirstNonBlank),
        KeyCode::Char('W') => Some(Command::MoveWORDForward),
        KeyCode::Char('B') => Some(Command::MoveWORDBackward),
        KeyCode::Char('E') => Some(Command::MoveWORDEnd),
        KeyCode::Char('{') => Some(Command::MoveParagraphBackward),
        KeyCode::Char('}') => Some(Command::MoveParagraphForward),
        KeyCode::Char('G') => Some(Command::GotoBottom),

        // Enter insert mode
        KeyCode::Char('i') => Some(Command::EnterInsertMode),
        KeyCode::Char('a') => Some(Command::EnterInsertModeAfter),
        KeyCode::Char('A') => Some(Command::EnterInsertModeLineEnd),
        KeyCode::Char('I') => Some(Command::EnterInsertModeFirstNonBlank),

        // Editing
        KeyCode::Char('o') => Some(Command::InsertNewlineBelow),
        KeyCode::Char('O') => Some(Command::InsertNewlineAbove),
        KeyCode::Char('x') => Some(Command::DeleteCharForward),
        KeyCode::Char('J') => Some(Command::JoinLines),
        KeyCode::Char('D') => Some(Command::DeleteMotion(Motion::LineEnd)),
        KeyCode::Char('C') => Some(Command::ChangeMotion(Motion::LineEnd)),

        // Repeat last change
        KeyCode::Char('.') => Some(Command::RepeatLastChange),

        // Toggle case of char under cursor
        KeyCode::Char('~') => Some(Command::ToggleCaseChar),

        // Search word under cursor
        KeyCode::Char('*') => Some(Command::SearchWordForward),
        KeyCode::Char('#') => Some(Command::SearchWordBackward),

        // Matching bracket
        KeyCode::Char('%') => Some(Command::MatchBracket),

        // Viewport navigation
        KeyCode::Char('H') => Some(Command::ViewportHigh),
        KeyCode::Char('M') => Some(Command::ViewportMiddle),
        KeyCode::Char('L') => Some(Command::ViewportLow),

        // Pending operators
        KeyCode::Char('d') => {
            editor.pending_keys.push('d');
            None
        }
        KeyCode::Char('c') => {
            editor.pending_keys.push('c');
            None
        }
        KeyCode::Char('y') => {
            editor.pending_keys.push('y');
            None
        }
        KeyCode::Char('g') => {
            editor.pending_keys.push('g');
            None
        }
        KeyCode::Char('>') => {
            editor.pending_keys.push('>');
            None
        }
        KeyCode::Char('<') => {
            editor.pending_keys.push('<');
            None
        }
        KeyCode::Char('f') => {
            editor.pending_keys.push('f');
            None
        }
        KeyCode::Char('F') => {
            editor.pending_keys.push('F');
            None
        }
        KeyCode::Char('t') => {
            editor.pending_keys.push('t');
            None
        }
        KeyCode::Char('T') => {
            editor.pending_keys.push('T');
            None
        }
        KeyCode::Char('r') => {
            editor.pending_keys.push('r');
            None
        }
        KeyCode::Char('z') => {
            editor.pending_keys.push('z');
            None
        }

        // Diagnostic/bracket prefix
        KeyCode::Char(']') => {
            editor.pending_keys.push(']');
            None
        }
        KeyCode::Char('[') => {
            editor.pending_keys.push('[');
            None
        }

        // Register prefix
        KeyCode::Char('"') => {
            editor.pending_keys.push('"');
            None
        }

        // Macro: q to start/stop, @ to play
        KeyCode::Char('q') => {
            if editor.recording_macro.is_some() {
                Some(Command::StopMacro)
            } else {
                editor.pending_keys.push('q');
                None
            }
        }
        KeyCode::Char('@') => {
            editor.pending_keys.push('@');
            None
        }

        // LSP: Hover
        KeyCode::Char('K') => Some(Command::Hover),

        // Paste
        KeyCode::Char('p') => Some(Command::PasteAfter),
        KeyCode::Char('P') => Some(Command::PasteBefore),

        // Undo
        KeyCode::Char('u') => Some(Command::Undo),

        // Visual mode
        KeyCode::Char('v') => Some(Command::EnterVisualMode),
        KeyCode::Char('V') => Some(Command::EnterVisualLineMode),

        // Search
        KeyCode::Char('/') => Some(Command::EnterSearchMode),
        KeyCode::Char('n') => Some(Command::SearchNext),
        KeyCode::Char('N') => Some(Command::SearchPrev),

        // Command mode
        KeyCode::Char(':') => Some(Command::EnterCommandMode),

        _ => None,
    }
}

fn handle_pending(editor: &mut Editor, key: KeyEvent) -> Option<Command> {
    // Esc always cancels pending
    if key.code == KeyCode::Esc {
        editor.pending_keys.clear();
        return None;
    }

    let ch = match key.code {
        KeyCode::Char(ch) => ch,
        _ => {
            editor.pending_keys.clear();
            return None;
        }
    };

    let pending = editor.pending_keys.clone();
    editor.pending_keys.clear();

    match pending.as_slice() {
        // --- Register prefix ---
        &['"'] => {
            editor.selected_register = Some(ch);
            None
        }

        // --- Macro ---
        &['q'] => Some(Command::StartMacro(ch)),
        &['@'] => {
            if ch == '@' {
                Some(Command::PlayLastMacro)
            } else {
                Some(Command::PlayMacro(ch))
            }
        }

        // --- Operators: d, c, y ---
        &['d'] => match ch {
            'd' => Some(Command::DeleteLine),
            'w' => Some(Command::DeleteMotion(Motion::WordForward)),
            'e' => Some(Command::DeleteMotion(Motion::WordEnd)),
            'b' => Some(Command::DeleteMotion(Motion::WordBackward)),
            'W' => Some(Command::DeleteMotion(Motion::WORDForward)),
            'E' => Some(Command::DeleteMotion(Motion::WORDEnd)),
            'B' => Some(Command::DeleteMotion(Motion::WORDBackward)),
            '$' => Some(Command::DeleteMotion(Motion::LineEnd)),
            '0' => Some(Command::DeleteMotion(Motion::LineStart)),
            '^' => Some(Command::DeleteMotion(Motion::FirstNonBlank)),
            '}' => Some(Command::DeleteMotion(Motion::ParagraphForward)),
            '{' => Some(Command::DeleteMotion(Motion::ParagraphBackward)),
            'i' | 'a' | 'f' | 'F' | 't' | 'T' => {
                editor.pending_keys.push('d');
                editor.pending_keys.push(ch);
                None
            }
            _ => None,
        },
        &['d', 'i'] => Some(Command::DeleteMotion(Motion::Inner(ch))),
        &['d', 'a'] => Some(Command::DeleteMotion(Motion::Around(ch))),
        &['d', 'f'] => Some(Command::DeleteMotion(Motion::FindForward(ch))),
        &['d', 'F'] => Some(Command::DeleteMotion(Motion::FindBackward(ch))),
        &['d', 't'] => Some(Command::DeleteMotion(Motion::TillForward(ch))),
        &['d', 'T'] => Some(Command::DeleteMotion(Motion::TillBackward(ch))),

        &['c'] => match ch {
            'c' => Some(Command::ChangeMotion(Motion::Line)),
            'w' => Some(Command::ChangeMotion(Motion::WordForward)),
            'e' => Some(Command::ChangeMotion(Motion::WordEnd)),
            'b' => Some(Command::ChangeMotion(Motion::WordBackward)),
            'W' => Some(Command::ChangeMotion(Motion::WORDForward)),
            'E' => Some(Command::ChangeMotion(Motion::WORDEnd)),
            'B' => Some(Command::ChangeMotion(Motion::WORDBackward)),
            '$' => Some(Command::ChangeMotion(Motion::LineEnd)),
            '0' => Some(Command::ChangeMotion(Motion::LineStart)),
            '^' => Some(Command::ChangeMotion(Motion::FirstNonBlank)),
            '}' => Some(Command::ChangeMotion(Motion::ParagraphForward)),
            '{' => Some(Command::ChangeMotion(Motion::ParagraphBackward)),
            'i' | 'a' | 'f' | 'F' | 't' | 'T' => {
                editor.pending_keys.push('c');
                editor.pending_keys.push(ch);
                None
            }
            _ => None,
        },
        &['c', 'i'] => Some(Command::ChangeMotion(Motion::Inner(ch))),
        &['c', 'a'] => Some(Command::ChangeMotion(Motion::Around(ch))),
        &['c', 'f'] => Some(Command::ChangeMotion(Motion::FindForward(ch))),
        &['c', 'F'] => Some(Command::ChangeMotion(Motion::FindBackward(ch))),
        &['c', 't'] => Some(Command::ChangeMotion(Motion::TillForward(ch))),
        &['c', 'T'] => Some(Command::ChangeMotion(Motion::TillBackward(ch))),

        &['y'] => match ch {
            'y' => Some(Command::YankLine),
            'w' => Some(Command::YankMotion(Motion::WordForward)),
            'e' => Some(Command::YankMotion(Motion::WordEnd)),
            'b' => Some(Command::YankMotion(Motion::WordBackward)),
            'W' => Some(Command::YankMotion(Motion::WORDForward)),
            'E' => Some(Command::YankMotion(Motion::WORDEnd)),
            'B' => Some(Command::YankMotion(Motion::WORDBackward)),
            '$' => Some(Command::YankMotion(Motion::LineEnd)),
            '0' => Some(Command::YankMotion(Motion::LineStart)),
            '^' => Some(Command::YankMotion(Motion::FirstNonBlank)),
            '}' => Some(Command::YankMotion(Motion::ParagraphForward)),
            '{' => Some(Command::YankMotion(Motion::ParagraphBackward)),
            'i' | 'a' => {
                editor.pending_keys.push('y');
                editor.pending_keys.push(ch);
                None
            }
            _ => None,
        },
        &['y', 'i'] => Some(Command::YankMotion(Motion::Inner(ch))),
        &['y', 'a'] => Some(Command::YankMotion(Motion::Around(ch))),

        // --- g-prefix ---
        &['g'] => match ch {
            'd' => Some(Command::GotoDefinition),
            'r' => Some(Command::FindReferences),
            'g' => Some(Command::GotoTop),
            'a' => Some(Command::CodeAction),
            'E' => Some(Command::DiagnosticList),
            't' => Some(Command::NextBuffer),
            'T' => Some(Command::PrevBuffer),
            // Case change: gu/gU/g~ + motion
            'u' | 'U' | '~' => {
                editor.pending_keys.push('g');
                editor.pending_keys.push(ch);
                None
            }
            _ => None,
        },

        // --- Case change: gu{motion}, gU{motion}, g~{motion} ---
        &['g', op @ ('u' | 'U' | '~')] => {
            map_case_motion(editor, op, ch)
        }
        &['g', 'u', 'i'] => Some(Command::CaseChange(CaseOp::Lower, Motion::Inner(ch))),
        &['g', 'u', 'a'] => Some(Command::CaseChange(CaseOp::Lower, Motion::Around(ch))),
        &['g', 'U', 'i'] => Some(Command::CaseChange(CaseOp::Upper, Motion::Inner(ch))),
        &['g', 'U', 'a'] => Some(Command::CaseChange(CaseOp::Upper, Motion::Around(ch))),
        &['g', '~', 'i'] => Some(Command::CaseChange(CaseOp::Toggle, Motion::Inner(ch))),
        &['g', '~', 'a'] => Some(Command::CaseChange(CaseOp::Toggle, Motion::Around(ch))),

        // --- Indent/dedent ---
        &['>'] => match ch {
            '>' => Some(Command::IndentLine),
            _ => None,
        },
        &['<'] => match ch {
            '<' => Some(Command::DedentLine),
            _ => None,
        },

        // --- Find/till character ---
        &['f'] => Some(Command::FindCharForward(ch)),
        &['F'] => Some(Command::FindCharBackward(ch)),
        &['t'] => Some(Command::TillCharForward(ch)),
        &['T'] => Some(Command::TillCharBackward(ch)),

        // --- Replace character ---
        &['r'] => Some(Command::ReplaceChar(ch)),

        // --- Diagnostic navigation ---
        &[']'] => match ch {
            'd' => Some(Command::DiagnosticNext),
            _ => None,
        },
        &['['] => match ch {
            'd' => Some(Command::DiagnosticPrev),
            _ => None,
        },

        // --- Window split (Ctrl-W prefix) ---
        &['W'] => match ch {
            'v' => Some(Command::SplitVertical),
            's' => Some(Command::SplitHorizontal),
            'h' => Some(Command::PaneLeft),
            'j' => Some(Command::PaneDown),
            'k' => Some(Command::PaneUp),
            'l' => Some(Command::PaneRight),
            'w' => Some(Command::PaneNext),
            'q' => Some(Command::PaneClose),
            _ => None,
        },

        // --- Scroll positioning ---
        &['z'] => match ch {
            'z' => Some(Command::ScrollCenter),
            't' => Some(Command::ScrollTop),
            'b' => Some(Command::ScrollBottom),
            _ => None,
        },

        _ => None,
    }
}

/// Helper for case change motions (gu/gU/g~ + motion key).
fn map_case_motion(editor: &mut Editor, op: char, ch: char) -> Option<Command> {
    let case_op = match op {
        'u' => CaseOp::Lower,
        'U' => CaseOp::Upper,
        '~' => CaseOp::Toggle,
        _ => return None,
    };
    match ch {
        c if c == op => Some(Command::CaseChangeLine(case_op)),
        'w' => Some(Command::CaseChange(case_op, Motion::WordForward)),
        'e' => Some(Command::CaseChange(case_op, Motion::WordEnd)),
        'b' => Some(Command::CaseChange(case_op, Motion::WordBackward)),
        'W' => Some(Command::CaseChange(case_op, Motion::WORDForward)),
        'E' => Some(Command::CaseChange(case_op, Motion::WORDEnd)),
        'B' => Some(Command::CaseChange(case_op, Motion::WORDBackward)),
        '$' => Some(Command::CaseChange(case_op, Motion::LineEnd)),
        '0' => Some(Command::CaseChange(case_op, Motion::LineStart)),
        '^' => Some(Command::CaseChange(case_op, Motion::FirstNonBlank)),
        'i' | 'a' => {
            editor.pending_keys.push('g');
            editor.pending_keys.push(op);
            editor.pending_keys.push(ch);
            None
        }
        _ => None,
    }
}

fn map_visual(key: KeyEvent) -> Option<Command> {
    // Ctrl-modified keys
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('d') => return Some(Command::HalfPageDown),
            KeyCode::Char('u') => return Some(Command::HalfPageUp),
            KeyCode::Char('f') => return Some(Command::FullPageDown),
            KeyCode::Char('b') => return Some(Command::FullPageUp),
            _ => return None,
        }
    }

    match key.code {
        // Movement
        KeyCode::Char('h') | KeyCode::Left => Some(Command::MoveLeft),
        KeyCode::Char('j') | KeyCode::Down => Some(Command::MoveDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Command::MoveUp),
        KeyCode::Char('l') | KeyCode::Right => Some(Command::MoveRight),
        KeyCode::Char('w') => Some(Command::MoveWordForward),
        KeyCode::Char('b') => Some(Command::MoveWordBackward),
        KeyCode::Char('e') => Some(Command::MoveWordEnd),
        KeyCode::Char('W') => Some(Command::MoveWORDForward),
        KeyCode::Char('B') => Some(Command::MoveWORDBackward),
        KeyCode::Char('E') => Some(Command::MoveWORDEnd),
        KeyCode::Char('0') => Some(Command::MoveLineStart),
        KeyCode::Char('$') => Some(Command::MoveLineEnd),
        KeyCode::Char('^') => Some(Command::MoveFirstNonBlank),
        KeyCode::Char('{') => Some(Command::MoveParagraphBackward),
        KeyCode::Char('}') => Some(Command::MoveParagraphForward),
        KeyCode::Char('G') => Some(Command::GotoBottom),

        // Swap anchor/cursor
        KeyCode::Char('o') => Some(Command::VisualSwapAnchor),

        // Case change on selection
        KeyCode::Char('~') => Some(Command::ToggleCaseChar),
        KeyCode::Char('u') => Some(Command::CaseChangeLine(CaseOp::Lower)),
        KeyCode::Char('U') => Some(Command::CaseChangeLine(CaseOp::Upper)),

        // Operations on selection
        KeyCode::Char('d') | KeyCode::Char('x') => Some(Command::VisualDelete),
        KeyCode::Char('y') => Some(Command::VisualYank),
        KeyCode::Char('c') => Some(Command::VisualChange),
        KeyCode::Char('>') => Some(Command::VisualIndent),
        KeyCode::Char('<') => Some(Command::VisualDedent),

        // Exit
        KeyCode::Esc | KeyCode::Char('v') | KeyCode::Char('V') => {
            Some(Command::ExitToNormalMode)
        }

        _ => None,
    }
}

fn map_insert(editor: &Editor, key: KeyEvent) -> Option<Command> {
    // When completion popup is showing, intercept navigation keys
    if editor.showing_completion {
        match key.code {
            KeyCode::Down | KeyCode::Tab => return Some(Command::CompletionNext),
            KeyCode::Up | KeyCode::BackTab => return Some(Command::CompletionPrev),
            KeyCode::Enter => return Some(Command::AcceptCompletion),
            KeyCode::Esc => return Some(Command::CancelCompletion),
            _ => {
                // Any other key dismisses completion and falls through
            }
        }
    }

    match key.code {
        KeyCode::Esc => Some(Command::ExitToNormalMode),
        KeyCode::Backspace => Some(Command::DeleteCharBackward),
        KeyCode::Enter => Some(Command::InsertNewline),
        KeyCode::Left => Some(Command::MoveLeft),
        KeyCode::Right => Some(Command::MoveRight),
        KeyCode::Up => Some(Command::MoveUp),
        KeyCode::Down => Some(Command::MoveDown),
        // Ctrl-Space to trigger completion
        KeyCode::Char(' ') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Command::TriggerCompletion)
        }
        KeyCode::Char(ch) => Some(Command::InsertChar(ch)),
        KeyCode::Tab => Some(Command::InsertTab),
        _ => None,
    }
}

fn map_command(key: KeyEvent) -> Option<Command> {
    match key.code {
        KeyCode::Esc => Some(Command::ExitToNormalMode),
        KeyCode::Enter => Some(Command::CommandExecute),
        KeyCode::Backspace => Some(Command::CommandBackspace),
        KeyCode::Up => Some(Command::CommandHistoryPrev),
        KeyCode::Down => Some(Command::CommandHistoryNext),
        KeyCode::Char(ch) => Some(Command::CommandInput(ch)),
        _ => None,
    }
}

fn map_search(key: KeyEvent) -> Option<Command> {
    match key.code {
        KeyCode::Esc => Some(Command::SearchCancel),
        KeyCode::Enter => Some(Command::SearchConfirm),
        KeyCode::Backspace => Some(Command::SearchBackspace),
        KeyCode::Char(ch) => Some(Command::SearchInput(ch)),
        _ => None,
    }
}

fn map_file_finder(key: KeyEvent) -> Option<Command> {
    match key.code {
        KeyCode::Esc => Some(Command::FileFinderCancel),
        KeyCode::Enter => Some(Command::FileFinderConfirm),
        KeyCode::Backspace => Some(Command::FileFinderBackspace),
        KeyCode::Down | KeyCode::Tab => Some(Command::FileFinderNext),
        KeyCode::Up | KeyCode::BackTab => Some(Command::FileFinderPrev),
        KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Command::FileFinderNext)
        }
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Command::FileFinderPrev)
        }
        KeyCode::Char(ch) => Some(Command::FileFinderInput(ch)),
        _ => None,
    }
}
