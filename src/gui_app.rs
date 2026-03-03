use std::path::Path;
use std::sync::mpsc as std_mpsc;

use crate::editor::{DeferredAction, Editor};
use crate::editor::document::Document;
use crate::editor::pane::AreaRect;
use crate::input;
use crate::input::command::Command;
use crate::input::keymap;
use crate::key::{KeyCode, KeyInput};
use crate::lsp::{self, LspClient, LspMessage};

pub struct GuiApp {
    editor: Editor,
    runtime: tokio::runtime::Runtime,
    lsp_client: Option<LspClient>,
    lsp_rx: std_mpsc::Receiver<LspMessage>,
    lsp_tx: std_mpsc::Sender<LspMessage>,
    file_uri: Option<String>,
    last_notified_version: i64,
    /// Pane pixel rects from last frame, used for mouse hit-testing.
    last_pane_rects: Vec<(usize, egui::Rect)>,
}

impl GuiApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, path: Option<String>) -> Self {
        let document = match path {
            Some(ref p) => Document::open(p).unwrap_or_else(|_| Document::new_empty()),
            None => Document::new_empty(),
        };

        let (lsp_tx, lsp_rx) = std_mpsc::channel();
        let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

        let mut app = Self {
            editor: Editor::new(document),
            runtime,
            lsp_client: None,
            lsp_rx,
            lsp_tx,
            file_uri: None,
            last_notified_version: 0,
            last_pane_rects: Vec::new(),
        };

        // Start LSP
        if let Some(ref p) = path {
            let path = std::path::PathBuf::from(p);
            if let Ok(canonical) = std::fs::canonicalize(&path) {
                app.editor.document.path = Some(canonical.clone());
                app.start_lsp(&canonical);
            }
        }

        app
    }

    fn start_lsp(&mut self, file_path: &Path) {
        let root = lsp::find_project_root(file_path);
        let tx = self.lsp_tx.clone();

        // Create a channel for the LSP async bridge
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();

        match self.runtime.block_on(LspClient::start(&root, event_tx)) {
            Ok(client) => {
                self.lsp_client = Some(client);
                self.editor.status_message = Some(format!("LSP: starting (root: {})", root.display()));

                // Spawn a task to bridge async LSP messages to sync channel
                self.runtime.spawn(async move {
                    while let Some(event) = event_rx.recv().await {
                        if let crate::lsp::AppEvent::Lsp(msg) = event {
                            if tx.send(msg).is_err() {
                                break;
                            }
                        }
                    }
                });
            }
            Err(e) => {
                self.editor.status_message = Some(format!("LSP: failed to start: {e}"));
            }
        }
    }

    fn process_lsp_messages(&mut self) {
        while let Ok(msg) = self.lsp_rx.try_recv() {
            self.handle_lsp_message(msg);
        }
    }

    fn handle_lsp_message(&mut self, msg: LspMessage) {
        match msg {
            LspMessage::Response { id, result, error } => {
                // Check if this is the initialize response
                let is_init = self.lsp_client.as_ref()
                    .is_some_and(|lsp| id == lsp.initialize_id && !lsp.initialized);

                if is_init {
                    if error.is_some() {
                        self.editor.status_message = Some("LSP: initialize failed".to_string());
                        return;
                    }
                    // Extract data before borrowing lsp_client mutably
                    let path_data = self.editor.document.path.as_ref().map(|path| {
                        let uri = lsp::path_to_uri(path);
                        let text = self.editor.document.rope.to_string();
                        let version = self.editor.document.version;
                        (uri, text, version)
                    });

                    if let Some(lsp) = &mut self.lsp_client {
                        let _ = self.runtime.block_on(lsp.send_initialized());
                        if let Some((ref uri, ref text, version)) = path_data {
                            let _ = self.runtime.block_on(lsp.did_open(uri, text, version));
                        }
                    }
                    if let Some((uri, _, _)) = path_data {
                        self.file_uri = Some(uri);
                    }
                    self.editor.status_message = Some("LSP: ready".to_string());
                    return;
                }

                // Completion
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

                // Goto definition
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
                            } else {
                                let name = loc.uri.rsplit('/').next().unwrap_or(&loc.uri);
                                self.editor.status_message = Some(format!(
                                    "Definition in {}:{}:{}",
                                    name, loc.start_line + 1, loc.start_col + 1
                                ));
                            }
                        } else {
                            self.editor.status_message = Some("No definition found".to_string());
                        }
                    }
                    return;
                }

                // Hover
                if Some(id) == self.editor.pending_hover_id {
                    self.editor.pending_hover_id = None;
                    if let Some(result) = result {
                        if let Some(text) = lsp::parse_hover(&result) {
                            self.editor.hover_text = Some(text);
                            self.editor.showing_hover = true;
                        } else {
                            self.editor.status_message = Some("No hover info".to_string());
                        }
                    }
                    return;
                }

                // References
                if Some(id) == self.editor.pending_references_id {
                    self.editor.pending_references_id = None;
                    if let Some(result) = result {
                        let locations = lsp::parse_references(&result);
                        if !locations.is_empty() {
                            self.editor.references = locations;
                            self.editor.reference_index = 0;
                            self.editor.showing_references = true;
                        } else {
                            self.editor.status_message = Some("No references found".to_string());
                        }
                    }
                    return;
                }

                // Format
                if Some(id) == self.editor.pending_format_id {
                    self.editor.pending_format_id = None;
                    if let Some(result) = result {
                        self.apply_format_edits(&result);
                    }
                    return;
                }

                // Code actions
                if Some(id) == self.editor.pending_code_action_id {
                    self.editor.pending_code_action_id = None;
                    if let Some(result) = result {
                        let actions = lsp::parse_code_actions(&result);
                        if !actions.is_empty() {
                            self.editor.code_actions = actions;
                            self.editor.code_action_index = 0;
                            self.editor.showing_code_actions = true;
                        } else {
                            self.editor.status_message = Some("No code actions available".to_string());
                        }
                    }
                    return;
                }

                // Rename
                if Some(id) == self.editor.pending_rename_id {
                    self.editor.pending_rename_id = None;
                    if let Some(ref err) = error {
                        let msg = err.get("message").and_then(|m| m.as_str()).unwrap_or("Rename failed");
                        self.editor.status_message = Some(format!("Rename error: {msg}"));
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
                if let Some(lsp) = &mut self.lsp_client {
                    if method == "window/workDoneProgress/create" || method == "client/registerCapability" {
                        let _ = self.runtime.block_on(lsp.respond(&id, serde_json::Value::Null));
                    }
                }
            }
        }
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
        self.editor.history.save(&self.editor.document.rope, self.editor.cursor);
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
        text_edits.sort_by(|a, b| (b.2, b.3).cmp(&(a.2, a.3)));
        for (start_line, start_col, end_line, end_col, new_text) in &text_edits {
            let line_count = self.editor.document.rope.len_lines();
            if *start_line >= line_count { continue; }
            let end_line = (*end_line).min(line_count.saturating_sub(1));
            let start_idx = self.editor.document.rope.line_to_char(*start_line) + start_col;
            let end_idx = self.editor.document.rope.line_to_char(end_line) + end_col;
            let end_idx = end_idx.min(self.editor.document.rope.len_chars());
            let start_idx = start_idx.min(self.editor.document.rope.len_chars());
            if start_idx < end_idx { self.editor.document.rope.remove(start_idx..end_idx); }
            if !new_text.is_empty() { self.editor.document.rope.insert(start_idx, new_text); }
        }
        self.editor.document.modified = true;
        self.editor.document.bump_version();
        self.editor.clamp_cursor();
        self.editor.status_message = Some("Formatted".to_string());
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
        self.editor.history.save(&self.editor.document.rope, self.editor.cursor);
        let count = edits.len();
        for edit in &edits {
            let start_line_char = self.editor.document.rope.line_to_char(edit.start_line as usize);
            let end_line_char = self.editor.document.rope.line_to_char(edit.end_line as usize);
            let start_idx = start_line_char + edit.start_col as usize;
            let end_idx = end_line_char + edit.end_col as usize;
            let end_idx = end_idx.min(self.editor.document.rope.len_chars());
            if start_idx < end_idx { self.editor.document.rope.remove(start_idx..end_idx); }
            if !edit.new_text.is_empty() { self.editor.document.rope.insert(start_idx, &edit.new_text); }
        }
        self.editor.document.modified = true;
        self.editor.document.bump_version();
        self.editor.clamp_cursor();
        self.editor.status_message = Some(format!("Renamed: {count} occurrence(s)"));
    }

    fn handle_key(&mut self, key: KeyInput, ctx: &egui::Context) {
        // Record for macros
        if self.editor.recording_macro.is_some() {
            let is_stop = matches!(key.code, KeyCode::Char('q'))
                && !key.ctrl
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
            if trigger_completion { self.request_completion(); }
            if trigger_goto { self.request_goto_definition(); }
            if trigger_hover { self.request_hover(); }
            if trigger_refs { self.request_references(); }
            if trigger_ref_jump { self.jump_to_reference(); }
            if trigger_file_finder {
                let entries = self.scan_project_files();
                self.editor.open_file_finder(entries);
            }
            if trigger_code_action { self.request_code_action(); }
            if trigger_code_action_accept { self.accept_code_action(); }

            // Handle deferred actions
            if let Some(action) = deferred {
                self.handle_deferred(action);
            }
        }

        // Send LSP didChange
        self.notify_lsp_change();

        ctx.request_repaint();
    }

    fn handle_deferred(&mut self, action: DeferredAction) {
        match action {
            DeferredAction::Rename(new_name) => { self.request_rename(&new_name); }
            DeferredAction::DidSave => { self.notify_lsp_did_save(); }
            DeferredAction::OpenFile(path) => { self.open_file(&path); }
            DeferredAction::SyncFileUri => { self.sync_file_uri(); }
            DeferredAction::ShellCommand(cmd) => {
                // Run shell command and capture output
                match std::process::Command::new("sh").arg("-c").arg(&cmd).output() {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let stderr_out = String::from_utf8_lossy(&output.stderr);
                        let msg = if !stdout.is_empty() {
                            stdout.lines().next().unwrap_or("").to_string()
                        } else if !stderr_out.is_empty() {
                            stderr_out.lines().next().unwrap_or("").to_string()
                        } else {
                            format!("Exit: {}", output.status.code().unwrap_or(-1))
                        };
                        self.editor.status_message = Some(msg);
                    }
                    Err(e) => {
                        self.editor.status_message = Some(format!("Error: {e}"));
                    }
                }
            }
            DeferredAction::FormatDocument => { self.request_formatting(); }
            DeferredAction::PlayMacro(ch) => { self.play_macro(ch); }
        }
    }

    fn handle_mouse(&mut self, ctx: &egui::Context) {
        let font_size = self.editor.config.gui_font_size;
        let char_width = font_size * 0.6;
        let line_height = font_size * 1.4;

        // --- Scroll wheel ---
        let scroll_delta = ctx.input(|i| i.smooth_scroll_delta);
        if scroll_delta.y.abs() > 0.1 {
            let lines = (scroll_delta.y.abs() / line_height).ceil() as usize;
            let lines = lines.max(3);
            if scroll_delta.y > 0.0 {
                self.editor.scroll_viewport_up(lines);
            } else {
                self.editor.scroll_viewport_down(lines);
            }
            ctx.request_repaint();
        }

        // --- Click ---
        let clicked = ctx.input(|i| i.pointer.primary_clicked());
        if !clicked {
            return;
        }
        let click_pos = match ctx.input(|i| i.pointer.interact_pos()) {
            Some(pos) => pos,
            None => return,
        };

        // Ignore clicks while popups are showing
        if self.editor.showing_completion
            || self.editor.showing_hover
            || self.editor.showing_references
            || self.editor.showing_code_actions
            || self.editor.showing_diagnostics
            || self.editor.showing_file_finder
        {
            return;
        }

        // Find which pane was clicked
        let clicked_pane = self.last_pane_rects.iter().find(|(_, rect)| rect.contains(click_pos));
        let (pane_id, pane_rect) = match clicked_pane {
            Some(&(id, rect)) => (id, rect),
            None => return,
        };

        // Switch pane focus if needed
        if pane_id != self.editor.active_pane_id {
            self.editor.save_active_pane();
            self.editor.load_pane(pane_id);
            self.sync_file_uri();
        }

        // Determine the editor area (pane minus status line)
        let gutter_width = self.editor.gutter_width();
        let gutter_px = gutter_width as f32 * char_width;
        let editor_rows = (self.editor.view.height) as f32 * line_height;
        let editor_area_bottom = pane_rect.min.y + editor_rows;

        // Click on status line → just focus the pane (already done above)
        if click_pos.y >= editor_area_bottom {
            ctx.request_repaint();
            return;
        }

        // Click on gutter → ignore
        if click_pos.x < pane_rect.min.x + gutter_px {
            ctx.request_repaint();
            return;
        }

        // Convert pixel position to screen coordinates
        let screen_col_f = (click_pos.x - pane_rect.min.x - gutter_px) / char_width;
        let screen_row_f = (click_pos.y - pane_rect.min.y) / line_height;
        let screen_col = screen_col_f.max(0.0) as usize;
        let screen_row = screen_row_f.max(0.0) as usize;

        // Convert screen coordinates to document coordinates
        if self.editor.config.wrap {
            let text_width = self.editor.view.width.saturating_sub(gutter_width);
            let screen_map = crate::editor::wrap::build_screen_map(
                &self.editor.document.rope,
                self.editor.view.offset_row,
                self.editor.view.offset_wrap,
                text_width,
                self.editor.view.height,
            );
            if let Some(seg) = screen_map.get(screen_row) {
                self.editor.cursor.row = seg.doc_row;
                self.editor.cursor.col = seg.char_start + screen_col;
            }
        } else {
            self.editor.cursor.row = self.editor.view.offset_row + screen_row;
            self.editor.cursor.col = self.editor.view.offset_col + screen_col;
        }

        self.editor.clamp_cursor();
        self.editor.scroll();
        ctx.request_repaint();
    }

    fn play_macro(&mut self, ch: char) {
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
                        DeferredAction::PlayMacro(_) => {}
                        other => self.handle_deferred(other),
                    }
                }
            }
            self.notify_lsp_change();
            if self.editor.should_quit { break; }
        }
    }

    // LSP request helpers (blocking via runtime)

    fn notify_lsp_change(&mut self) {
        let version = self.editor.document.version;
        if version == self.last_notified_version {
            return;
        }
        if let (Some(lsp), Some(uri)) = (&mut self.lsp_client, &self.file_uri) {
            if !lsp.initialized { return; }
            let text = self.editor.document.rope.to_string();
            let _ = self.runtime.block_on(lsp.did_change(uri, &text, version));
            self.last_notified_version = version;
        }
    }

    fn notify_lsp_did_save(&mut self) {
        if let (Some(lsp), Some(uri)) = (&mut self.lsp_client, &self.file_uri) {
            if !lsp.initialized { return; }
            let _ = self.runtime.block_on(lsp.did_save(uri));
        }
    }

    fn request_completion(&mut self) {
        if let (Some(lsp), Some(uri)) = (&mut self.lsp_client, &self.file_uri) {
            if !lsp.initialized { return; }
            let line = self.editor.cursor.row as u32;
            let character = self.editor.cursor.col as u32;
            if let Ok(id) = self.runtime.block_on(lsp.completion(uri, line, character)) {
                self.editor.pending_completion_id = Some(id);
            }
        }
    }

    fn request_goto_definition(&mut self) {
        if let (Some(lsp), Some(uri)) = (&mut self.lsp_client, &self.file_uri) {
            if !lsp.initialized { return; }
            let line = self.editor.cursor.row as u32;
            let character = self.editor.cursor.col as u32;
            if let Ok(id) = self.runtime.block_on(lsp.goto_definition(uri, line, character)) {
                self.editor.pending_goto_id = Some(id);
            }
        }
    }

    fn request_hover(&mut self) {
        if let (Some(lsp), Some(uri)) = (&mut self.lsp_client, &self.file_uri) {
            if !lsp.initialized { return; }
            let line = self.editor.cursor.row as u32;
            let character = self.editor.cursor.col as u32;
            if let Ok(id) = self.runtime.block_on(lsp.hover(uri, line, character)) {
                self.editor.pending_hover_id = Some(id);
            }
        }
    }

    fn request_references(&mut self) {
        if let (Some(lsp), Some(uri)) = (&mut self.lsp_client, &self.file_uri) {
            if !lsp.initialized { return; }
            let line = self.editor.cursor.row as u32;
            let character = self.editor.cursor.col as u32;
            if let Ok(id) = self.runtime.block_on(lsp.references(uri, line, character)) {
                self.editor.pending_references_id = Some(id);
            }
        }
    }

    fn request_rename(&mut self, new_name: &str) {
        if let (Some(lsp), Some(uri)) = (&mut self.lsp_client, &self.file_uri) {
            if !lsp.initialized { return; }
            let line = self.editor.cursor.row as u32;
            let character = self.editor.cursor.col as u32;
            if let Ok(id) = self.runtime.block_on(lsp.rename(uri, line, character, new_name)) {
                self.editor.pending_rename_id = Some(id);
            }
        }
    }

    fn request_code_action(&mut self) {
        if let (Some(lsp), Some(uri)) = (&mut self.lsp_client, &self.file_uri) {
            if !lsp.initialized { return; }
            let line = self.editor.cursor.row as u32;
            let character = self.editor.cursor.col as u32;
            let diagnostics = self.editor.diagnostics.clone();
            if let Ok(id) = self.runtime.block_on(lsp.code_action(uri, line, character, &diagnostics)) {
                self.editor.pending_code_action_id = Some(id);
            }
        }
    }

    fn accept_code_action(&mut self) {
        if self.editor.code_actions.is_empty() {
            self.editor.dismiss_code_actions();
            return;
        }
        let action = self.editor.code_actions[self.editor.code_action_index].clone();
        self.editor.dismiss_code_actions();
        if let Some(ref edit) = action.edit {
            self.apply_workspace_edit(edit);
        }
        self.editor.status_message = Some(format!("Applied: {}", action.title));
        self.notify_lsp_change();
    }

    fn apply_workspace_edit(&mut self, edit: &serde_json::Value) {
        let file_uri = match &self.file_uri {
            Some(uri) => uri.clone(),
            None => return,
        };
        let mut text_edits: Vec<lsp::LspTextEdit> = Vec::new();
        if let Some(changes) = edit.get("changes").and_then(|c| c.as_object()) {
            if let Some(file_edits) = changes.get(&file_uri).and_then(|e| e.as_array()) {
                for e in file_edits {
                    if let Some(te) = lsp::parse_text_edit(e) { text_edits.push(te); }
                }
            }
        }
        if text_edits.is_empty() {
            if let Some(doc_changes) = edit.get("documentChanges").and_then(|c| c.as_array()) {
                for dc in doc_changes {
                    let uri = dc.get("textDocument").and_then(|td| td.get("uri")).and_then(|u| u.as_str());
                    if uri == Some(&file_uri) {
                        if let Some(edit_arr) = dc.get("edits").and_then(|e| e.as_array()) {
                            for e in edit_arr {
                                if let Some(te) = lsp::parse_text_edit(e) { text_edits.push(te); }
                            }
                        }
                    }
                }
            }
        }
        if text_edits.is_empty() { return; }
        self.editor.history.save(&self.editor.document.rope, self.editor.cursor);
        text_edits.sort_by(|a, b| b.start_line.cmp(&a.start_line).then(b.start_col.cmp(&a.start_col)));
        for te in &text_edits {
            let line_count = self.editor.document.rope.len_lines();
            if (te.start_line as usize) >= line_count { continue; }
            let end_line = (te.end_line as usize).min(line_count.saturating_sub(1));
            let start_idx = self.editor.document.rope.line_to_char(te.start_line as usize) + te.start_col as usize;
            let end_idx = self.editor.document.rope.line_to_char(end_line) + te.end_col as usize;
            let end_idx = end_idx.min(self.editor.document.rope.len_chars());
            let start_idx = start_idx.min(self.editor.document.rope.len_chars());
            if start_idx < end_idx { self.editor.document.rope.remove(start_idx..end_idx); }
            if !te.new_text.is_empty() { self.editor.document.rope.insert(start_idx, &te.new_text); }
        }
        self.editor.document.modified = true;
        self.editor.document.bump_version();
        self.editor.clamp_cursor();
    }

    fn request_formatting(&mut self) {
        if let (Some(lsp), Some(uri)) = (&mut self.lsp_client, &self.file_uri) {
            if !lsp.initialized { return; }
            let params = serde_json::json!({
                "textDocument": { "uri": uri },
                "options": { "tabSize": 4, "insertSpaces": true }
            });
            match self.runtime.block_on(lsp.send_request("textDocument/formatting", params)) {
                Ok(id) => { self.editor.pending_format_id = Some(id); }
                Err(_) => { self.editor.status_message = Some("Format request failed".to_string()); }
            }
        }
    }

    fn jump_to_reference(&mut self) {
        if self.editor.references.is_empty() { return; }
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
            self.editor.status_message = Some(format!("Reference in {}:{}:{}", name, loc.start_line + 1, loc.start_col + 1));
            self.editor.dismiss_popup();
        }
    }

    fn sync_file_uri(&mut self) {
        if let (Some(lsp), Some(old_uri)) = (&mut self.lsp_client, &self.file_uri) {
            if lsp.initialized {
                let _ = self.runtime.block_on(lsp.send_notification(
                    "textDocument/didClose",
                    serde_json::json!({"textDocument": {"uri": old_uri}}),
                ));
            }
        }
        // Reset version tracking for the new file
        self.last_notified_version = self.editor.document.version;
        if let Some(path) = &self.editor.document.path {
            if let Some(lsp) = &mut self.lsp_client {
                if lsp.initialized {
                    let uri = lsp::path_to_uri(path);
                    let text = self.editor.document.rope.to_string();
                    let version = self.editor.document.version;
                    let _ = self.runtime.block_on(lsp.did_open(&uri, &text, version));
                    self.file_uri = Some(uri);
                }
            }
        } else {
            self.file_uri = None;
        }
    }

    fn open_file(&mut self, rel_path: &str) {
        let root = if let Some(path) = &self.editor.document.path {
            lsp::find_project_root(path)
        } else {
            std::env::current_dir().unwrap_or_default()
        };
        let full_path = root.join(rel_path);
        if let Some(idx) = self.editor.find_buffer_by_path(&full_path) {
            if idx != self.editor.current_buffer {
                self.editor.switch_buffer(idx);
                self.sync_file_uri();
            }
            return;
        }
        match Document::open(&full_path.to_string_lossy()) {
            Ok(doc) => {
                self.editor.add_buffer(doc);
                self.editor.status_message = Some(format!("\"{}\"", self.editor.document.file_name()));
                // Reset version tracking for the new file
                self.last_notified_version = self.editor.document.version;
                if let Some(lsp) = &mut self.lsp_client {
                    if lsp.initialized {
                        let uri = lsp::path_to_uri(&full_path);
                        let text = self.editor.document.rope.to_string();
                        let version = self.editor.document.version;
                        let _ = self.runtime.block_on(lsp.did_open(&uri, &text, version));
                        self.file_uri = Some(uri);
                    }
                }
            }
            Err(e) => {
                self.editor.status_message = Some(format!("Error opening file: {e}"));
            }
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
            if name_str.starts_with('.') { continue; }
            if matches!(name_str.as_ref(), "target" | "node_modules" | "build" | "dist" | "__pycache__") { continue; }
            if path.is_dir() {
                Self::walk_dir(root, &path, out);
            } else if path.is_file() {
                if let Ok(rel) = path.strip_prefix(root) {
                    out.push(rel.to_string_lossy().to_string());
                }
            }
        }
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process pending LSP messages
        self.process_lsp_messages();

        // Handle keyboard input
        let events: Vec<egui::Event> = ctx.input(|i| i.events.clone());
        for event in &events {
            match event {
                egui::Event::Key { key, pressed: true, modifiers, .. } => {
                    let key_input = egui_key_to_key_input(*key, *modifiers);
                    if let Some(ki) = key_input {
                        self.handle_key(ki, ctx);
                    }
                }
                egui::Event::Text(text) => {
                    // Text events are for character input (Insert mode, command mode, etc.)
                    // Only process if we're in a mode that accepts text input
                    // and the key wasn't already handled as a special key
                    if matches!(self.editor.mode,
                        crate::input::mode::Mode::Insert
                        | crate::input::mode::Mode::Command
                        | crate::input::mode::Mode::Search
                    ) {
                        for ch in text.chars() {
                            if !ch.is_control() {
                                let ki = KeyInput { code: KeyCode::Char(ch), ctrl: false };
                                self.handle_key(ki, ctx);
                            }
                        }
                    } else {
                        // Normal/Visual mode: process characters as commands
                        for ch in text.chars() {
                            if !ch.is_control() {
                                let ki = KeyInput { code: KeyCode::Char(ch), ctrl: false };
                                self.handle_key(ki, ctx);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Update viewport dimensions
        let avail = ctx.available_rect();
        let font_size = self.editor.config.gui_font_size;
        let char_width = font_size * 0.6;
        let line_height = font_size * 1.4;
        let cols = (avail.width() / char_width) as u16;
        let rows = (avail.height() / line_height) as u16;

        let tab_rows: u16 = if self.editor.buffers.len() > 1 { 1 } else { 0 };
        let command_rows: u16 = 1;
        let pane_area = AreaRect::new(0, tab_rows, cols, rows.saturating_sub(tab_rows + command_rows));
        self.editor.editor_area = pane_area;

        let pane_rects = self.editor.pane_layout.layout(pane_area);
        for &(pane_id, rect) in &pane_rects {
            let editor_height = rect.height.saturating_sub(1);
            let editor_width = rect.width;
            if pane_id == self.editor.active_pane_id {
                self.editor.view.width = editor_width;
                self.editor.view.height = editor_height;
            } else {
                if let Some(pane) = self.editor.panes.iter_mut().find(|p| p.id == pane_id) {
                    pane.view.width = editor_width;
                    pane.view.height = editor_height;
                }
            }
        }

        self.editor.scroll();
        self.editor.update_highlights();

        // Handle mouse input (uses pane rects from previous frame)
        self.handle_mouse(ctx);

        // Render and save pane rects for next frame's mouse handling
        self.last_pane_rects = crate::gui::render(&self.editor, ctx);

        // Quit
        if self.editor.should_quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        // Request repaint for continuous updates (LSP messages, etc.)
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

/// Convert egui Key + Modifiers to our KeyInput.
/// Returns None for keys that will be handled via Text events (character input).
fn egui_key_to_key_input(key: egui::Key, modifiers: egui::Modifiers) -> Option<KeyInput> {
    let ctrl = modifiers.ctrl || modifiers.mac_cmd;

    // Special keys that are always handled
    let code = match key {
        egui::Key::Escape => Some(KeyCode::Esc),
        egui::Key::Enter => Some(KeyCode::Enter),
        egui::Key::Backspace => Some(KeyCode::Backspace),
        egui::Key::Tab => {
            if modifiers.shift {
                Some(KeyCode::BackTab)
            } else {
                Some(KeyCode::Tab)
            }
        }
        egui::Key::ArrowUp => Some(KeyCode::Up),
        egui::Key::ArrowDown => Some(KeyCode::Down),
        egui::Key::ArrowLeft => Some(KeyCode::Left),
        egui::Key::ArrowRight => Some(KeyCode::Right),
        // Ctrl+key combinations
        _ if ctrl => {
            // Map letter keys with ctrl
            match key {
                egui::Key::A => Some(KeyCode::Char('a')),
                egui::Key::B => Some(KeyCode::Char('b')),
                egui::Key::C => Some(KeyCode::Char('c')),
                egui::Key::D => Some(KeyCode::Char('d')),
                egui::Key::F => Some(KeyCode::Char('f')),
                egui::Key::I => Some(KeyCode::Char('i')),
                egui::Key::O => Some(KeyCode::Char('o')),
                egui::Key::P => Some(KeyCode::Char('p')),
                egui::Key::R => Some(KeyCode::Char('r')),
                egui::Key::U => Some(KeyCode::Char('u')),
                egui::Key::W => Some(KeyCode::Char('w')),
                egui::Key::X => Some(KeyCode::Char('x')),
                egui::Key::Space => Some(KeyCode::Char(' ')),
                _ => None,
            }
        }
        // Regular character keys will be handled by Text events
        _ => None,
    };

    code.map(|c| KeyInput { code: c, ctrl })
}
