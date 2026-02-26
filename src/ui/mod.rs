pub mod code_actions;
pub mod command_line;
pub mod completion;
pub mod diagnostics;
pub mod editor_view;
pub mod file_finder;
pub mod hover;
pub mod references;
pub mod status_line;
pub mod tab_bar;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};

use crate::editor::Editor;

use self::code_actions::CodeActionsPopup;
use self::command_line::CommandLine;
use self::completion::CompletionPopup;
use self::diagnostics::DiagnosticsPopup;
use self::editor_view::EditorView;
use self::file_finder::FileFinderPopup;
use self::hover::HoverPopup;
use self::references::ReferencesPopup;
use self::status_line::StatusLine;
use self::tab_bar::TabBar;

pub fn render(editor: &Editor, frame: &mut Frame) {
    let size = frame.area();

    let show_tabs = editor.buffers.len() > 1;

    let chunks = if show_tabs {
        Layout::vertical([
            Constraint::Length(1), // tab bar
            Constraint::Min(1),    // editor area
            Constraint::Length(1), // status line
            Constraint::Length(1), // command line
        ])
        .split(size)
    } else {
        Layout::vertical([
            Constraint::Length(0), // no tab bar
            Constraint::Min(1),    // editor area
            Constraint::Length(1), // status line
            Constraint::Length(1), // command line
        ])
        .split(size)
    };

    if show_tabs {
        frame.render_widget(TabBar::new(editor), chunks[0]);
    }
    frame.render_widget(EditorView::new(editor), chunks[1]);
    // Popups render over the editor area
    frame.render_widget(CompletionPopup::new(editor), chunks[1]);
    frame.render_widget(HoverPopup::new(editor), chunks[1]);
    frame.render_widget(ReferencesPopup::new(editor), chunks[1]);
    frame.render_widget(CodeActionsPopup::new(editor), chunks[1]);
    frame.render_widget(DiagnosticsPopup::new(editor), chunks[1]);
    frame.render_widget(FileFinderPopup::new(editor), chunks[1]);
    frame.render_widget(StatusLine::new(editor), chunks[2]);
    frame.render_widget(CommandLine::new(editor), chunks[3]);
}
