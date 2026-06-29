// Hide the console window on Windows release builds (GUI app).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod bulk;
mod docx;
mod docx_merge;
mod export;
mod filenamer;
mod help;
mod placeholder;
mod theme;
mod xlsx;

use std::sync::Arc;

use app::App;

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1120.0, 840.0])
        .with_min_inner_size([980.0, 740.0])
        .with_title("LetterFactory");

    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon_256.png")) {
        viewport = viewport.with_icon(Arc::new(icon));
    }

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "LetterFactory",
        native_options,
        Box::new(|cc| {
            theme::apply(&cc.egui_ctx);
            Ok(Box::new(App::new(&cc.egui_ctx)))
        }),
    )
}
