//! ScreenForge desktop app — Rust + egui.
//!
//! Default launch matches the original app: a **tray application**. No main
//! window — a global hotkey opens the region overlay, you annotate, it saves.
//! The full windowed UI (settings, GIF editor, translate) opens only when asked
//! (`--window`), or from the tray menu.
mod app;
mod icons;
mod panels;
mod theme;
mod tray;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--window" || a == "-w") {
        return run_window();
    }
    // Default: tray + hotkey + overlay, exactly like the C# app.
    tray::run_tray();
    Ok(())
}

/// Opens the full windowed UI.
fn run_window() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([980.0, 640.0])
            .with_min_inner_size([720.0, 480.0])
            .with_title("ScreenForge"),
        ..Default::default()
    };
    eframe::run_native(
        "ScreenForge",
        native_options,
        Box::new(|cc| Ok(Box::new(app::ScreenForgeApp::new(cc)) as Box<dyn eframe::App>)),
    )
}
