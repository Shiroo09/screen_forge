//! Translate panel ("Çeviri"): source text, target language, background translation.

use crate::app::ScreenForgeApp;
use crate::theme;
use egui::{Color32, RichText};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

type Outcome = Result<sf_net::translate::Translation, String>;

/// Result of the most recent background translation, picked up by `ui()`.
fn slot() -> &'static Arc<Mutex<Option<Outcome>>> {
    static SLOT: OnceLock<Arc<Mutex<Option<Outcome>>>> = OnceLock::new();
    SLOT.get_or_init(|| Arc::new(Mutex::new(None)))
}

/// True while a worker thread is translating.
fn busy() -> &'static AtomicBool {
    static BUSY: OnceLock<AtomicBool> = OnceLock::new();
    BUSY.get_or_init(|| AtomicBool::new(false))
}

/// Source language reported by the last translation.
fn detected() -> &'static Mutex<Option<String>> {
    static DETECTED: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    DETECTED.get_or_init(|| Mutex::new(None))
}

pub fn ui(app: &mut ScreenForgeApp, ui: &mut egui::Ui) {
    poll(app);

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.heading("Çeviri");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(&app.status).color(theme::TEXT_DIM).size(12.0));
        });
    });
    ui.label(
        RichText::new("Metni yaz, hedef dili seç; çeviri arka planda hazırlanır.")
            .color(theme::TEXT_DIM),
    );
    ui.add_space(10.0);

    // --- Target language ----------------------------------------------------
    let current = target_lang(app);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Hedef dil").color(theme::TEXT).strong());
        egui::ComboBox::from_id_salt("translate_target")
            .selected_text(sf_settings::label(&current))
            .show_ui(ui, |ui| {
                for (code, label) in sf_settings::LANGUAGES {
                    let selected = *code == current.as_str();
                    if ui.selectable_label(selected, *label).clicked() {
                        app.settings.translate_native_language = (*code).to_string();
                    }
                }
            });
    });
    ui.add_space(8.0);

    // --- Input --------------------------------------------------------------
    egui::Frame::new()
        .fill(theme::SURFACE)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .inner_margin(egui::Margin::same(10))
        .corner_radius(8.0)
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut app.translate_input)
                    .hint_text("Çevrilecek metni buraya yazın…")
                    .desired_rows(7)
                    .desired_width(f32::INFINITY),
            );
        });

    ui.add_space(10.0);

    // --- Translate button ---------------------------------------------------
    let running = busy().load(Ordering::SeqCst);
    let label = if running { "Çevriliyor…" } else { "Çevir" };
    let button = egui::Button::new(
        RichText::new(label)
            .size(15.0)
            .strong()
            .color(Color32::WHITE),
    )
    .fill(theme::ACCENT)
    .stroke(egui::Stroke::new(1.0, theme::BORDER))
    .corner_radius(8.0)
    .min_size(egui::vec2(168.0, 44.0));

    if ui.add_enabled(!running, button).clicked() {
        start(app);
    }
    if running {
        // Keep polling for the worker's result while the frame is idle.
        ui.ctx().request_repaint();
    }

    ui.add_space(10.0);

    // --- Output -------------------------------------------------------------
    egui::Frame::new()
        .fill(theme::SURFACE)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .inner_margin(egui::Margin::same(10))
        .corner_radius(8.0)
        .show(ui, |ui| {
            ui.set_min_height(140.0);
            ui.set_min_width(ui.available_width());

            if app.translate_output.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(48.0);
                    ui.label(RichText::new("Çeviri burada görünecek").color(theme::TEXT_DIM));
                });
                return;
            }

            ui.horizontal(|ui| {
                ui.label(RichText::new("Çeviri").strong().color(theme::TEXT));
                if let Some(code) = detected().lock().ok().and_then(|g| g.clone()) {
                    ui.label(
                        RichText::new(format!("kaynak dil: {}", sf_settings::label(&code)))
                            .color(theme::TEXT_DIM)
                            .size(12.0),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Panoya kopyala").clicked() {
                        ui.ctx().copy_text(app.translate_output.clone());
                        app.notify("panoya kopyalandı");
                    }
                });
            });
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .max_height(220.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(RichText::new(&app.translate_output).color(theme::TEXT))
                            .wrap()
                            .selectable(true),
                    );
                });
        });
}

/// Target language code, falling back to Turkish when unset.
fn target_lang(app: &ScreenForgeApp) -> String {
    let code = app.settings.translate_native_language.trim();
    if code.is_empty() {
        "tr".to_string()
    } else {
        code.to_lowercase()
    }
}

/// Starts the background translation for the current input.
fn start(app: &mut ScreenForgeApp) {
    let text = app.translate_input.trim().to_string();
    if text.is_empty() {
        app.notify("önce çevrilecek metni yaz");
        return;
    }

    let target = target_lang(app);
    let slot = slot().clone();
    let busy = busy();
    app.status = "çevriliyor...".to_string();
    busy.store(true, Ordering::SeqCst);

    std::thread::spawn(move || {
        let outcome = sf_net::translate::translate(&text, &target);
        if let Ok(mut guard) = slot.lock() {
            *guard = Some(outcome);
        }
        busy.store(false, Ordering::SeqCst);
    });
}

/// Moves a finished translation out of the shared slot into the app state.
fn poll(app: &mut ScreenForgeApp) {
    let Some(outcome) = slot().lock().ok().and_then(|mut guard| guard.take()) else {
        return;
    };
    match outcome {
        Ok(t) => {
            app.translate_output = t.text;
            app.status = "çevrildi".to_string();
            if let Ok(mut guard) = detected().lock() {
                *guard = t.detected_language;
            }
            app.notify("çeviri hazır");
        }
        Err(e) => {
            app.status = "hazır".to_string();
            app.notify(format!("hata: {e}"));
        }
    }
}
