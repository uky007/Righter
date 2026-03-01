pub mod editor_view;
pub mod status_line;
pub mod command_line;
pub mod completion;
pub mod hover;
pub mod references;
pub mod code_actions;
pub mod diagnostics;
pub mod file_finder;
pub mod tab_bar;

use egui::{CentralPanel, TopBottomPanel, Color32, Frame as EguiFrame, Margin};

use crate::editor::Editor;
use crate::editor::pane::AreaRect;


use self::editor_view::draw_editor_view;
use self::status_line::draw_status_line;
use self::command_line::draw_command_line;
use self::completion::draw_completion;
use self::hover::draw_hover;
use self::references::draw_references;
use self::code_actions::draw_code_actions;
use self::diagnostics::draw_diagnostics;
use self::file_finder::draw_file_finder;
use self::tab_bar::draw_tab_bar;

/// One Dark background color.
const BG_COLOR: Color32 = Color32::from_rgb(40, 44, 52);

/// Render the full editor UI using egui.
pub fn render(editor: &Editor, ctx: &egui::Context) {
    let show_tabs = editor.buffers.len() > 1;

    // Tab bar at top
    if show_tabs {
        TopBottomPanel::top("tab_bar").show(ctx, |ui| {
            draw_tab_bar(editor, ui);
        });
    }

    // Command line at bottom
    TopBottomPanel::bottom("command_line").show(ctx, |ui| {
        draw_command_line(editor, ui);
    });

    // Main editor area
    CentralPanel::default()
        .frame(EguiFrame::new().fill(BG_COLOR).inner_margin(Margin::ZERO))
        .show(ctx, |ui| {
            let rect = ui.available_rect_before_wrap();
            let font_size = 14.0;
            let char_width = font_size * 0.6;
            let line_height = font_size * 1.4;

            // Calculate pane layout using the editor's AreaRect system
            let cols = (rect.width() / char_width) as u16;
            let rows = (rect.height() / line_height) as u16;

            let pane_area = AreaRect::new(0, 0, cols, rows);
            let pane_rects = editor.pane_layout.layout(pane_area);

            for &(pane_id, arect) in &pane_rects {
                if arect.height < 2 {
                    continue;
                }
                let is_active = pane_id == editor.active_pane_id;

                // Convert AreaRect to screen coordinates
                let pane_rect = egui::Rect::from_min_size(
                    egui::pos2(
                        rect.min.x + arect.x as f32 * char_width,
                        rect.min.y + arect.y as f32 * line_height,
                    ),
                    egui::vec2(
                        arect.width as f32 * char_width,
                        arect.height as f32 * line_height,
                    ),
                );

                let editor_rows = arect.height.saturating_sub(1);
                let editor_rect = egui::Rect::from_min_size(
                    pane_rect.min,
                    egui::vec2(pane_rect.width(), editor_rows as f32 * line_height),
                );
                let status_rect = egui::Rect::from_min_size(
                    egui::pos2(pane_rect.min.x, pane_rect.min.y + editor_rows as f32 * line_height),
                    egui::vec2(pane_rect.width(), line_height),
                );

                draw_editor_view(editor, pane_id, is_active, ui, editor_rect, char_width, line_height);
                draw_status_line(editor, pane_id, is_active, ui, status_rect, char_width, line_height);
            }

            // Draw popups over active pane
            let active_arect = pane_rects
                .iter()
                .find(|(id, _)| *id == editor.active_pane_id)
                .map(|(_, r)| *r)
                .unwrap_or(pane_area);

            let popup_rect = egui::Rect::from_min_size(
                egui::pos2(
                    rect.min.x + active_arect.x as f32 * char_width,
                    rect.min.y + active_arect.y as f32 * line_height,
                ),
                egui::vec2(
                    active_arect.width as f32 * char_width,
                    active_arect.height.saturating_sub(1) as f32 * line_height,
                ),
            );

            draw_completion(editor, ui, popup_rect, char_width, line_height);
            draw_hover(editor, ui, popup_rect, char_width, line_height);
            draw_references(editor, ui, popup_rect, char_width, line_height);
            draw_code_actions(editor, ui, popup_rect, char_width, line_height);
            draw_diagnostics(editor, ui, popup_rect, char_width, line_height);
            draw_file_finder(editor, ui, popup_rect, char_width, line_height);
        });
}
