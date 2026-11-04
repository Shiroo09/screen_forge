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
        let tab = args
            .iter()
            .position(|a| a == "--tab")
            .and_then(|i| args.get(i + 1))
            .and_then(|t| match t.as_str() {
                "capture" => Some(app::Tab::Capture),
                "annotate" => Some(app::Tab::Annotate),
                "record" => Some(app::Tab::Record),
                "gif" => Some(app::Tab::Gif),
                "translate" => Some(app::Tab::Translate),
                "settings" => Some(app::Tab::Settings),
                _ => None,
            });
        return run_window(tab);
    }
    // Default: tray + hotkey + overlay, exactly like the C# app.
    tray::run_tray();
    Ok(())
}

/// Opens the full windowed UI, optionally on a specific tab.
fn run_window(tab: Option<app::Tab>) -> eframe::Result<()> {
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
        Box::new(move |cc| {
            let mut app = app::ScreenForgeApp::new(cc);
            if let Some(t) = tab {
                app.tab = t;
            }
            Ok(Box::new(app) as Box<dyn eframe::App>)
        }),
    )
}
