use std::io::{Stderr, Write as _};
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyEventKind};
use futures::StreamExt;
use ratatui::Terminal;
use ratatui::prelude::CrosstermBackend;
use tokio::sync::mpsc;

use crate::editor::{DeferredAction, Editor};
use crate::editor::document::Document;
use crate::input;
use crate::input::command::Command;
use crate::input::keymap;
use crate::lsp::{self, AppEvent, LspClient, LspMessage};
use crate::ui;

pub struct App {
    pub editor: Editor,
    lsp_client: Option<LspClient>,
    event_rx: mpsc::UnboundedReceiver<AppEvent>,
    event_tx: mpsc::UnboundedSender<AppEvent>,
    file_uri: Option<String>,
}

impl App {
    pub fn new(path: Option<String>) -> Result<Self> {
        let document = match path {
            Some(p) => Document::open(&p)?,
            None => Document::new_empty(),
        };

        let (event_tx, event_rx) = mpsc::unbounded_channel();

        Ok(Self {
            editor: Editor::new(document),
            lsp_client: None,
            event_rx,
            event_tx,
            file_uri: None,
        })
    }

    pub async fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<Stderr>>) -> Result<()> {
        // Start LSP if we have a file with a path
        if let Some(path) = &self.editor.document.path {
            let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.clone());
            self.editor.document.path = Some(path.clone());
            self.start_lsp(&path).await;
        }

        // Spawn crossterm event reader into the event channel
        let tx = self.event_tx.clone();
        tokio::spawn(async move {
            let mut reader = EventStream::new();
            loop {
                match reader.next().await {
                    Some(Ok(Event::Key(key))) => {
                        if key.kind == KeyEventKind::Press {
                            if tx.send(AppEvent::Key(key)).is_err() {
                                break;
                            }
                        }
                    }
                    Some(Ok(Event::Resize(w, h))) => {
                        if tx.send(AppEvent::Resize(w, h)).is_err() {
                            break;
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                    None => break,
                }
            }
        });

        // Render tick interval
        let mut render_interval = tokio::time::interval(Duration::from_millis(16));

        loop {
            tokio::select! {
                _ = render_interval.tick() => {
                    // Update view dimensions and scroll
                    let size = terminal.size()?;
                    self.editor.view.width = size.width;
                    let tab_rows: u16 = if self.editor.buffers.len() > 1 { 1 } else { 0 };
                    self.editor.view.height = size.height.saturating_sub(2 + tab_rows);
                    self.editor.scroll();

                    // Update syntax highlights for visible viewport
                    self.editor.update_highlights();

                    // Render
                    terminal.draw(|frame| {
                        ui::render(&self.editor, frame);
                    })?;
                }

                Some(event) = self.event_rx.recv() => {
                    match event {
                        AppEvent::Key(key) => {
                            // Record key events for macro recording
                            if self.editor.recording_macro.is_some() {
                                // Don't record the 'q' that stops recording
                                let is_stop = matches!(
                                    key.code,
                                    crossterm::event::KeyCode::Char('q')
                                ) && key.modifiers.is_empty()
                                    && self.editor.mode == crate::input::mode::Mode::Normal;
                                if !is_stop {
                                    self.editor.macro_buffer.push(key);
                                }
                            }

                            if let Some(cmd) = keymap::map_key(&mut self.editor, key) {
                                let trigger_completion = matches!(cmd, Command::TriggerCompletion);
                                let trigger_goto = matches!(cmd, Command::GotoDefinition);
                                let trigger_hover = matches!(cmd, Command::Hover);
                                let trigger_refs = matches!(cmd, Command::FindReferences);
                                let trigger_ref_jump = matches!(cmd, Command::ReferenceJump);
                                let trigger_file_finder = matches!(cmd, Command::OpenFileFinder);
                                let trigger_code_action = matches!(cmd, Command::CodeAction);
                                let trigger_code_action_accept = matches!(cmd, Command::CodeActionAccept);

                                // Dismiss completion on non-completion input
                                if !matches!(
                                    cmd,
                                    Command::TriggerCompletion
                                    | Command::AcceptCompletion
                                    | Command::CancelCompletion
                                    | Command::CompletionNext
                                    | Command::CompletionPrev
                                ) && self.editor.showing_completion {
                                    self.editor.cancel_completion();
                                }

                                let deferred = input::execute(&mut self.editor, cmd);

                                // Handle async LSP commands
                                if trigger_completion {
                                    self.request_completion().await;
                                }
                                if trigger_goto {
                                    self.request_goto_definition().await;
                                }
                                if trigger_hover {
                                    self.request_hover().await;
                                }
                                if trigger_refs {
                                    self.request_references().await;
                                }
                                if trigger_ref_jump {
                                    self.jump_to_reference();
                                }
                                if trigger_file_finder {
                                    let entries = self.scan_project_files();
                                    self.editor.open_file_finder(entries);
                                }
                                if trigger_code_action {
                                    self.request_code_action().await;
                                }
                                if trigger_code_action_accept {
                                    self.accept_code_action().await;
                                }

                                // Handle deferred actions
                                if let Some(action) = deferred {
                                    self.handle_deferred(action, terminal).await;
                                }
                            }

                            // Send LSP didChange after edits
                            self.notify_lsp_change().await;
                        }

                        AppEvent::Resize(_, _) => {
                            // Size will be updated on next render tick
                        }

                        AppEvent::Lsp(msg) => {
                            self.handle_lsp_message(msg).await;
                        }
                    }

                    if self.editor.should_quit {
                        break;
                    }
                }
            }
        }

        // Shutdown LSP
        if let Some(lsp) = &mut self.lsp_client {
            let _ = lsp.shutdown().await;
        }

        Ok(())
    }

    async fn start_lsp(&mut self, file_path: &Path) {
        let root = lsp::find_project_root(file_path);
        let tx = self.event_tx.clone();

        match LspClient::start(&root, tx).await {
            Ok(client) => {
                self.lsp_client = Some(client);
                self.editor.status_message =
                    Some(format!("LSP: starting (root: {})", root.display()));
            }
            Err(e) => {
                self.editor.status_message =
                    Some(format!("LSP: failed to start: {e}"));
            }
        }
    }

    async fn handle_lsp_message(&mut self, msg: LspMessage) {
        match msg {
            LspMessage::Response { id, result, error } => {
                // Handle initialize response
                if let Some(lsp) = &mut self.lsp_client {
                    if id == lsp.initialize_id && !lsp.initialized {
                        if error.is_some() {
                            self.editor.status_message =
                                Some("LSP: initialize failed".to_string());
                            return;
                        }
                        let _ = lsp.send_initialized().await;

                        // Send didOpen for the current file
                        if let Some(path) = &self.editor.document.path {
                            let uri = lsp::path_to_uri(path);
                            let text = self.editor.document.rope.to_string();
                            let version = self.editor.document.version;
                            let _ = lsp.did_open(&uri, &text, version).await;
                            self.file_uri = Some(uri);
                        }

                        self.editor.status_message = Some("LSP: ready".to_string());
                        return;
                    }
                }

                // Handle completion response
                if Some(id) == self.editor.pending_completion_id {
                    self.editor.pending_completion_id = None;
                    if let Some(result) = result {
                        let items = lsp::parse_completions(&result);
                        if !items.is_empty() {
                            self.editor.completions = items;
                            self.editor.completion_index = 0;
                            self.editor.showing_completion = true;
                        }
                    }
                    return;
                }

                // Handle goto definition response
                if Some(id) == self.editor.pending_goto_id {
                    self.editor.pending_goto_id = None;
                    if let Some(result) = result {
                        let locations = lsp::parse_goto_definition(&result);
                        if let Some(loc) = locations.first() {
                            self.editor.push_jump();
                            let current_uri = self.file_uri.as_deref().unwrap_or("");
                            if loc.uri == current_uri {
                                self.editor.cursor.row = loc.start_line as usize;
                                self.editor.cursor.col = loc.start_col as usize;
                                self.editor.clamp_cursor();
                                self.editor.scroll();
                            } else if let Some(path) = lsp::uri_to_path(&loc.uri) {
                                let target_line = loc.start_line;
                                let target_col = loc.start_col;
                                self.open_file(&path).await;
                                self.editor.cursor.row = target_line as usize;
                                self.editor.cursor.col = target_col as usize;
                                self.editor.clamp_cursor();
                                self.editor.scroll();
                            } else {
                                let name = loc.uri.rsplit('/').next().unwrap_or(&loc.uri);
                                self.editor.status_message = Some(format!(
                                    "Definition in {}:{}:{}",
                                    name,
                                    loc.start_line + 1,
                                    loc.start_col + 1
                                ));
                            }
                        } else {
                            self.editor.status_message =
                                Some("No definition found".to_string());
                        }
                    }
                    return;
                }

                // Handle hover response
                if Some(id) == self.editor.pending_hover_id {
                    self.editor.pending_hover_id = None;
                    if let Some(result) = result {
                        if let Some(text) = lsp::parse_hover(&result) {
                            self.editor.hover_text = Some(text);
                            self.editor.showing_hover = true;
                        } else {
                            self.editor.status_message =
                                Some("No hover info".to_string());
                        }
                    }
                    return;
                }

                // Handle references response
                if Some(id) == self.editor.pending_references_id {
                    self.editor.pending_references_id = None;
                    if let Some(result) = result {
                        let locations = lsp::parse_references(&result);
                        if !locations.is_empty() {
                            self.editor.references = locations;
                            self.editor.reference_index = 0;
                            self.editor.showing_references = true;
                        } else {
                            self.editor.status_message =
                                Some("No references found".to_string());
                        }
                    }
                    return;
                }

                // Handle formatting response
                if Some(id) == self.editor.pending_format_id {
                    self.editor.pending_format_id = None;
                    if let Some(result) = result {
                        self.apply_format_edits(&result);
                    }
                    return;
                }

                // Handle code action response
                if Some(id) == self.editor.pending_code_action_id {
                    self.editor.pending_code_action_id = None;
                    if let Some(result) = result {
                        let actions = lsp::parse_code_actions(&result);
                        if !actions.is_empty() {
                            self.editor.code_actions = actions;
                            self.editor.code_action_index = 0;
                            self.editor.showing_code_actions = true;
                        } else {
                            self.editor.status_message =
                                Some("No code actions available".to_string());
                        }
                    }
                    return;
                }

                // Handle rename response
                if Some(id) == self.editor.pending_rename_id {
                    self.editor.pending_rename_id = None;
                    if let Some(ref err) = error {
                        let msg = err
                            .get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("Rename failed");
                        self.editor.status_message =
                            Some(format!("Rename error: {msg}"));
                        return;
                    }
                    if let Some(result) = result {
                        self.apply_rename_edits(&result);
                    }
                }
            }

            LspMessage::Notification { method, params } => {
                if method == "textDocument/publishDiagnostics" {
                    self.editor.diagnostics = lsp::parse_diagnostics(&params);
                }
            }

            LspMessage::ServerRequest { id, method, .. } => {
                // Respond to server requests (e.g., window/workDoneProgress/create)
                if let Some(lsp) = &mut self.lsp_client {
                    if method == "window/workDoneProgress/create"
                        || method == "client/registerCapability"
                    {
                        let _ = lsp.respond(&id, serde_json::Value::Null).await;
                    }
                }
            }
        }
    }

    async fn notify_lsp_change(&mut self) {
        if let (Some(lsp), Some(uri)) = (&mut self.lsp_client, &self.file_uri) {
            if !lsp.initialized {
                return;
            }
            let text = self.editor.document.rope.to_string();
            let version = self.editor.document.version;
            let _ = lsp.did_change(uri, &text, version).await;
        }
    }

    async fn request_completion(&mut self) {
        if let (Some(lsp), Some(uri)) = (&mut self.lsp_client, &self.file_uri) {
            if !lsp.initialized {
                return;
            }
            let line = self.editor.cursor.row as u32;
            let character = self.editor.cursor.col as u32;
            if let Ok(id) = lsp.completion(uri, line, character).await {
                self.editor.pending_completion_id = Some(id);
            }
        }
    }

    async fn request_goto_definition(&mut self) {
        if let (Some(lsp), Some(uri)) = (&mut self.lsp_client, &self.file_uri) {
            if !lsp.initialized {
                return;
            }
            let line = self.editor.cursor.row as u32;
            let character = self.editor.cursor.col as u32;
            if let Ok(id) = lsp.goto_definition(uri, line, character).await {
                self.editor.pending_goto_id = Some(id);
            }
        }
    }

    async fn request_hover(&mut self) {
        if let (Some(lsp), Some(uri)) = (&mut self.lsp_client, &self.file_uri) {
            if !lsp.initialized {
                return;
            }
            let line = self.editor.cursor.row as u32;
            let character = self.editor.cursor.col as u32;
            if let Ok(id) = lsp.hover(uri, line, character).await {
                self.editor.pending_hover_id = Some(id);
            }
        }
    }

    async fn request_references(&mut self) {
        if let (Some(lsp), Some(uri)) = (&mut self.lsp_client, &self.file_uri) {
            if !lsp.initialized {
                return;
            }
            let line = self.editor.cursor.row as u32;
            let character = self.editor.cursor.col as u32;
            if let Ok(id) = lsp.references(uri, line, character).await {
                self.editor.pending_references_id = Some(id);
            }
        }
    }

    async fn request_rename(&mut self, new_name: &str) {
        if let (Some(lsp), Some(uri)) = (&mut self.lsp_client, &self.file_uri) {
            if !lsp.initialized {
                return;
            }
            let line = self.editor.cursor.row as u32;
            let character = self.editor.cursor.col as u32;
            if let Ok(id) = lsp.rename(uri, line, character, new_name).await {
                self.editor.pending_rename_id = Some(id);
            }
        }
    }

    async fn request_code_action(&mut self) {
        if let (Some(lsp), Some(uri)) = (&mut self.lsp_client, &self.file_uri) {
            if !lsp.initialized {
                return;
            }
            let line = self.editor.cursor.row as u32;
            let character = self.editor.cursor.col as u32;
            let diagnostics = self.editor.diagnostics.clone();
            if let Ok(id) = lsp.code_action(uri, line, character, &diagnostics).await {
                self.editor.pending_code_action_id = Some(id);
            }
        }
    }

    async fn accept_code_action(&mut self) {
        if self.editor.code_actions.is_empty() {
            self.editor.dismiss_code_actions();
            return;
        }
        let action = self.editor.code_actions[self.editor.code_action_index].clone();
        self.editor.dismiss_code_actions();

        // Apply workspace edit if present
        if let Some(ref edit) = action.edit {
            self.apply_workspace_edit(edit);
        }

        // Execute command if present
        if let Some(ref command) = action.command {
            if let Some(lsp) = &mut self.lsp_client {
                if lsp.initialized {
                    let cmd_str = command
                        .get("command")
                        .and_then(|c| c.as_str())
                        .unwrap_or("");
                    let arguments = command
                        .get("arguments")
                        .cloned()
                        .unwrap_or(serde_json::Value::Array(Vec::new()));
                    let _ = lsp
                        .send_request(
                            "workspace/executeCommand",
                            serde_json::json!({
                                "command": cmd_str,
                                "arguments": arguments,
                            }),
                        )
                        .await;
                }
            }
        }

        self.editor.status_message = Some(format!("Applied: {}", action.title));
        self.notify_lsp_change().await;
    }

    fn apply_workspace_edit(&mut self, edit: &serde_json::Value) {
        let file_uri = match &self.file_uri {
            Some(uri) => uri.clone(),
            None => return,
        };

        // Collect edits from "changes" or "documentChanges"
        let mut text_edits: Vec<lsp::LspTextEdit> = Vec::new();

        if let Some(changes) = edit.get("changes").and_then(|c| c.as_object()) {
            if let Some(file_edits) = changes.get(&file_uri).and_then(|e| e.as_array()) {
                for e in file_edits {
                    if let Some(te) = lsp::parse_text_edit(e) {
                        text_edits.push(te);
                    }
                }
            }
        }

        if text_edits.is_empty() {
            if let Some(doc_changes) = edit.get("documentChanges").and_then(|c| c.as_array()) {
                for dc in doc_changes {
                    let uri = dc
                        .get("textDocument")
                        .and_then(|td| td.get("uri"))
                        .and_then(|u| u.as_str());
                    if uri == Some(&file_uri) {
                        if let Some(edit_arr) = dc.get("edits").and_then(|e| e.as_array()) {
                            for e in edit_arr {
                                if let Some(te) = lsp::parse_text_edit(e) {
                                    text_edits.push(te);
                                }
                            }
                        }
                    }
                }
            }
        }

        if text_edits.is_empty() {
            return;
        }

        // Save undo before applying
        self.editor
            .history
            .save(&self.editor.document.rope, self.editor.cursor);

        // Sort in reverse order for safe application
        text_edits.sort_by(|a, b| {
            b.start_line
                .cmp(&a.start_line)
                .then(b.start_col.cmp(&a.start_col))
        });

        for te in &text_edits {
            let line_count = self.editor.document.rope.len_lines();
            if (te.start_line as usize) >= line_count {
                continue;
            }
            let end_line = (te.end_line as usize).min(line_count.saturating_sub(1));
            let start_idx = self.editor.document.rope.line_to_char(te.start_line as usize)
                + te.start_col as usize;
            let end_idx =
                self.editor.document.rope.line_to_char(end_line) + te.end_col as usize;
            let end_idx = end_idx.min(self.editor.document.rope.len_chars());
            let start_idx = start_idx.min(self.editor.document.rope.len_chars());
            if start_idx < end_idx {
                self.editor.document.rope.remove(start_idx..end_idx);
            }
            if !te.new_text.is_empty() {
                self.editor
                    .document
                    .rope
                    .insert(start_idx, &te.new_text);
            }
        }

        self.editor.document.modified = true;
        self.editor.document.bump_version();
        self.editor.clamp_cursor();
    }

    fn jump_to_reference(&mut self) {
        if self.editor.references.is_empty() {
            return;
        }
        let loc = &self.editor.references[self.editor.reference_index];
        let current_uri = self.file_uri.as_deref().unwrap_or("");
        if loc.uri == current_uri {
            self.editor.cursor.row = loc.start_line as usize;
            self.editor.cursor.col = loc.start_col as usize;
            self.editor.clamp_cursor();
            self.editor.scroll();
            self.editor.dismiss_popup();
        } else {
            let name = loc.uri.rsplit('/').next().unwrap_or(&loc.uri);
            self.editor.status_message = Some(format!(
                "Reference in {}:{}:{}",
                name,
                loc.start_line + 1,
                loc.start_col + 1
            ));
            self.editor.dismiss_popup();
        }
    }

    fn apply_rename_edits(&mut self, result: &serde_json::Value) {
        let file_uri = match &self.file_uri {
            Some(uri) => uri.clone(),
            None => return,
        };
        let edits = lsp::parse_rename_edits(result, &file_uri);
        if edits.is_empty() {
            self.editor.status_message = Some("No edits to apply".to_string());
            return;
        }

        // Save undo before applying all edits
        self.editor.history.save(&self.editor.document.rope, self.editor.cursor);

        let count = edits.len();
        // Edits are already sorted in reverse order by parse_rename_edits
        for edit in &edits {
            let start_line_char = self.editor.document.rope.line_to_char(edit.start_line as usize);
            let end_line_char = self.editor.document.rope.line_to_char(edit.end_line as usize);
            let start_idx = start_line_char + edit.start_col as usize;
            let end_idx = end_line_char + edit.end_col as usize;
            let end_idx = end_idx.min(self.editor.document.rope.len_chars());
            if start_idx < end_idx {
                self.editor.document.rope.remove(start_idx..end_idx);
            }
            if !edit.new_text.is_empty() {
                self.editor.document.rope.insert(start_idx, &edit.new_text);
            }
        }

        self.editor.document.modified = true;
        self.editor.document.bump_version();
        self.editor.clamp_cursor();
        self.editor.status_message = Some(format!("Renamed: {count} occurrence(s)"));
    }

    fn apply_format_edits(&mut self, result: &serde_json::Value) {
        let edits = match result.as_array() {
            Some(arr) => arr,
            None => {
                self.editor.status_message = Some("Formatted (no changes)".to_string());
                return;
            }
        };
        if edits.is_empty() {
            self.editor.status_message = Some("Formatted (no changes)".to_string());
            return;
        }

        // Save undo before applying
        self.editor.history.save(&self.editor.document.rope, self.editor.cursor);

        // Parse and sort edits in reverse order (apply from bottom to top)
        let mut text_edits: Vec<(usize, usize, usize, usize, String)> = edits
            .iter()
            .filter_map(|edit| {
                let range = edit.get("range")?;
                let start = range.get("start")?;
                let end = range.get("end")?;
                let new_text = edit.get("newText")?.as_str()?.to_string();
                Some((
                    start.get("line")?.as_u64()? as usize,
                    start.get("character")?.as_u64()? as usize,
                    end.get("line")?.as_u64()? as usize,
                    end.get("character")?.as_u64()? as usize,
                    new_text,
                ))
            })
            .collect();

        // Sort in reverse order by position
        text_edits.sort_by(|a, b| (b.2, b.3).cmp(&(a.2, a.3)));

        for (start_line, start_col, end_line, end_col, new_text) in &text_edits {
            let line_count = self.editor.document.rope.len_lines();
            if *start_line >= line_count {
                continue;
            }
            let end_line = (*end_line).min(line_count.saturating_sub(1));
            let start_idx = self.editor.document.rope.line_to_char(*start_line) + start_col;
            let end_idx = self.editor.document.rope.line_to_char(end_line) + end_col;
            let end_idx = end_idx.min(self.editor.document.rope.len_chars());
            let start_idx = start_idx.min(self.editor.document.rope.len_chars());
            if start_idx < end_idx {
                self.editor.document.rope.remove(start_idx..end_idx);
            }
            if !new_text.is_empty() {
                self.editor.document.rope.insert(start_idx, new_text);
            }
        }

        self.editor.document.modified = true;
        self.editor.document.bump_version();
        self.editor.clamp_cursor();
        self.editor.status_message = Some("Formatted".to_string());
    }

    async fn notify_lsp_did_save(&mut self) {
        if let (Some(lsp), Some(uri)) = (&mut self.lsp_client, &self.file_uri) {
            if !lsp.initialized {
                return;
            }
            let _ = lsp.did_save(uri).await;
        }
    }

    fn scan_project_files(&self) -> Vec<String> {
        let root = if let Some(path) = &self.editor.document.path {
            lsp::find_project_root(path)
        } else {
            std::env::current_dir().unwrap_or_default()
        };

        let mut files = Vec::new();
        Self::walk_dir(&root, &root, &mut files);
        files.sort();
        files
    }

    fn walk_dir(root: &Path, dir: &Path, out: &mut Vec<String>) {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Skip hidden dirs and common large/irrelevant dirs
            if name_str.starts_with('.') {
                continue;
            }
            if matches!(
                name_str.as_ref(),
                "target" | "node_modules" | "build" | "dist" | "__pycache__"
            ) {
                continue;
            }

            if path.is_dir() {
                Self::walk_dir(root, &path, out);
            } else if path.is_file() {
                if let Ok(rel) = path.strip_prefix(root) {
                    out.push(rel.to_string_lossy().to_string());
                }
            }
        }
    }

    async fn sync_file_uri(&mut self) {
        // Close old file in LSP
        if let (Some(lsp), Some(old_uri)) = (&mut self.lsp_client, &self.file_uri) {
            if lsp.initialized {
                let _ = lsp
                    .send_notification(
                        "textDocument/didClose",
                        serde_json::json!({
                            "textDocument": { "uri": old_uri }
                        }),
                    )
                    .await;
            }
        }

        // Open new file in LSP
        if let Some(path) = &self.editor.document.path {
            if let Some(lsp) = &mut self.lsp_client {
                if lsp.initialized {
                    let uri = lsp::path_to_uri(path);
                    let text = self.editor.document.rope.to_string();
                    let version = self.editor.document.version;
                    let _ = lsp.did_open(&uri, &text, version).await;
                    self.file_uri = Some(uri);
                }
            }
        } else {
            self.file_uri = None;
        }
    }

    async fn open_file(&mut self, rel_path: &str) {
        let root = if let Some(path) = &self.editor.document.path {
            lsp::find_project_root(path)
        } else {
            std::env::current_dir().unwrap_or_default()
        };

        let full_path = root.join(rel_path);

        // Check if already open in a buffer
        if let Some(idx) = self.editor.find_buffer_by_path(&full_path) {
            if idx != self.editor.current_buffer {
                self.editor.switch_buffer(idx);
                self.sync_file_uri().await;
            }
            return;
        }

        // Close old file in LSP
        if let (Some(lsp), Some(old_uri)) = (&mut self.lsp_client, &self.file_uri) {
            if lsp.initialized {
                let _ = lsp
                    .send_notification(
                        "textDocument/didClose",
                        serde_json::json!({
                            "textDocument": { "uri": old_uri }
                        }),
                    )
                    .await;
            }
        }

        // Open new document as a new buffer
        match Document::open(&full_path.to_string_lossy()) {
            Ok(doc) => {
                self.editor.add_buffer(doc);
                self.editor.status_message =
                    Some(format!("\"{}\"", self.editor.document.file_name()));

                // Open in LSP
                if let Some(lsp) = &mut self.lsp_client {
                    if lsp.initialized {
                        let uri = lsp::path_to_uri(&full_path);
                        let text = self.editor.document.rope.to_string();
                        let version = self.editor.document.version;
                        let _ = lsp.did_open(&uri, &text, version).await;
                        self.file_uri = Some(uri);
                    }
                }
            }
            Err(e) => {
                self.editor.status_message = Some(format!("Error opening file: {e}"));
            }
        }
    }

    async fn handle_deferred(
        &mut self,
        action: DeferredAction,
        terminal: &mut Terminal<CrosstermBackend<Stderr>>,
    ) {
        match action {
            DeferredAction::Rename(new_name) => {
                self.request_rename(&new_name).await;
            }
            DeferredAction::DidSave => {
                self.notify_lsp_did_save().await;
            }
            DeferredAction::OpenFile(path) => {
                self.open_file(&path).await;
            }
            DeferredAction::SyncFileUri => {
                self.sync_file_uri().await;
            }
            DeferredAction::ShellCommand(cmd) => {
                self.run_shell_command(&cmd, terminal);
            }
            DeferredAction::FormatDocument => {
                self.request_formatting().await;
            }
            DeferredAction::PlayMacro(ch) => {
                self.play_macro(ch, terminal).await;
            }
        }
    }

    fn run_shell_command(
        &mut self,
        cmd: &str,
        terminal: &mut Terminal<CrosstermBackend<Stderr>>,
    ) {
        // Leave alternate screen and disable raw mode
        let _ = crossterm::execute!(
            std::io::stderr(),
            crossterm::terminal::LeaveAlternateScreen
        );
        let _ = crossterm::terminal::disable_raw_mode();

        // Run the command
        let status = std::process::Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .status();

        // Show result and wait for Enter
        match status {
            Ok(s) => {
                eprintln!("\n[Process exited with {}]", s.code().unwrap_or(-1));
            }
            Err(e) => {
                eprintln!("\n[Error: {e}]");
            }
        }
        eprint!("Press ENTER to continue...");
        let _ = std::io::stderr().flush();

        // Wait for Enter key (blocking, raw mode is off)
        let mut buf = [0u8; 1];
        let _ = std::io::Read::read(&mut std::io::stdin(), &mut buf);

        // Re-enter alternate screen and raw mode
        let _ = crossterm::terminal::enable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stderr(),
            crossterm::terminal::EnterAlternateScreen
        );

        // Force a full redraw
        let _ = terminal.clear();
    }

    async fn request_formatting(&mut self) {
        if let (Some(lsp), Some(uri)) = (&mut self.lsp_client, &self.file_uri) {
            if !lsp.initialized {
                self.editor.status_message = Some("LSP not ready".to_string());
                return;
            }
            let params = serde_json::json!({
                "textDocument": { "uri": uri },
                "options": {
                    "tabSize": 4,
                    "insertSpaces": true,
                }
            });
            match lsp.send_request("textDocument/formatting", params).await {
                Ok(id) => {
                    self.editor.pending_format_id = Some(id);
                }
                Err(_) => {
                    self.editor.status_message = Some("Format request failed".to_string());
                }
            }
        } else {
            self.editor.status_message = Some("LSP not available".to_string());
        }
    }

    async fn play_macro(
        &mut self,
        ch: char,
        terminal: &mut Terminal<CrosstermBackend<Stderr>>,
    ) {
        let keys = match self.editor.macros.get(&ch) {
            Some(keys) => keys.clone(),
            None => {
                self.editor.status_message = Some(format!("Macro @{ch} is empty"));
                return;
            }
        };
        self.editor.last_macro = Some(ch);

        for key in &keys {
            if let Some(cmd) = keymap::map_key(&mut self.editor, *key) {
                let deferred = input::execute(&mut self.editor, cmd);
                if let Some(action) = deferred {
                    match action {
                        DeferredAction::PlayMacro(_) => {} // avoid infinite recursion
                        DeferredAction::Rename(new_name) => {
                            self.request_rename(&new_name).await;
                        }
                        DeferredAction::DidSave => {
                            self.notify_lsp_did_save().await;
                        }
                        DeferredAction::OpenFile(path) => {
                            self.open_file(&path).await;
                        }
                        DeferredAction::SyncFileUri => {
                            self.sync_file_uri().await;
                        }
                        DeferredAction::ShellCommand(cmd) => {
                            self.run_shell_command(&cmd, terminal);
                        }
                        DeferredAction::FormatDocument => {
                            self.request_formatting().await;
                        }
                    }
                }
            }
            self.notify_lsp_change().await;

            if self.editor.should_quit {
                break;
            }
        }
    }
}
