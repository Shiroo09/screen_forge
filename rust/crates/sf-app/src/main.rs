//! ScreenForge desktop app — Rust + egui.
mod app;
mod panels;
mod theme;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([860.0, 580.0])
            .with_title("ScreenForge"),
        ..Default::default()
    };
    eframe::run_native(
        "ScreenForge",
        native_options,
        Box::new(|cc| Ok(Box::new(app::ScreenForgeApp::new(cc)) as Box<dyn eframe::App>)),
    )
}
