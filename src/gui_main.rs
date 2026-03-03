mod buffer;
mod config;
mod editor;
mod gui;
mod gui_app;
mod highlight;
mod input;
mod key;
mod lsp;

use anyhow::Result;

fn main() -> Result<()> {
    let path = std::env::args().nth(1);
    let config_result = config::Config::load();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Righter")
            .with_inner_size([1024.0, 768.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Righter",
        native_options,
        Box::new(move |cc| Ok(Box::new(gui_app::GuiApp::new(cc, path, config_result)))),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}
