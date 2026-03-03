#[cfg(feature = "tui")]
mod app;
mod buffer;
mod config;
mod editor;
mod highlight;
mod input;
mod key;
mod lsp;
#[cfg(feature = "tui")]
mod ui;

use std::io::stderr;

use anyhow::Result;
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::prelude::CrosstermBackend;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stderr_handle = stderr();
    execute!(stderr_handle, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stderr());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // Get file path from args
    let path = std::env::args().nth(1);

    // Load config
    let config_result = config::Config::load();

    // Run app
    let mut app = app::App::new(path, config_result)?;
    let result = app.run(&mut terminal).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}
