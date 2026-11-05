//! Cloud upload ("Buluta yükle"): sends the current capture to Litterbox and
//! shows the share link. The request runs on a worker thread so the UI keeps
//! drawing; the result travels back through a shared slot.

use crate::app::ScreenForgeApp;
use crate::theme;
use egui::RichText;
use std::sync::{Arc, Mutex, OnceLock};

/// Result of the last background upload, picked up by `ui()`.
fn slot() -> &'static Arc<Mutex<Option<Result<String, String>>>> {
    type UploadSlot = Arc<Mutex<Option<Result<String, String>>>>;
    static SLOT: OnceLock<UploadSlot> = OnceLock::new();
    SLOT.get_or_init(|| Arc::new(Mutex::new(None)))
}

/// When the share box should disappear on its own, for
/// `auto_close_upload_window`. `None` while the box should stay.
fn hide_at() -> &'static Mutex<Option<std::time::Instant>> {
    static HIDE: OnceLock<Mutex<Option<std::time::Instant>>> = OnceLock::new();
    HIDE.get_or_init(|| Mutex::new(None))
}

/// Encodes a BGRA buffer to PNG bytes, swapping B/R to RGBA on the way.
fn encode_png(width: i32, height: i32, bgra: &[u8]) -> Result<Vec<u8>, String> {
    if width <= 0 || height <= 0 || bgra.len() < (width * height * 4) as usize {
        return Err("geçersiz görüntü tamponu".to_string());
    }
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for px in bgra.chunks_exact(4) {
        rgba.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
    }
    let img = image::RgbaImage::from_raw(width as u32, height as u32, rgba)
        .ok_or_else(|| "görüntü boyutu uyuşmuyor".to_string())?;
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .map_err(|e| format!("png: {e}"))?;
    Ok(out)
}

/// Starts uploading the current capture on a worker thread. No-op while one is
/// already in flight, so a double click cannot open two requests.
pub fn start(app: &mut ScreenForgeApp) {
    if app.uploading {
        return;
    }
    let Some(img) = &app.captured else {
        app.notify("önce bir görüntü yakala");
        return;
    };
    let (width, height, bgra) = (img.width, img.height, img.bgra.clone());
    let slot = slot().clone();
    app.uploading = true;
    app.status = "yükleniyor...".to_string();

    std::thread::spawn(move || {
        // Encode off the UI thread too: a 4K PNG takes long enough to stutter.
        let outcome = encode_png(width, height, &bgra)
            .and_then(|png| sf_net::http_upload::upload(&png, "image/png", None));
        if let Ok(mut guard) = slot.lock() {
            *guard = Some(outcome);
        }
    });
}

/// Polls a finished upload and draws the share-link box.
pub fn ui(app: &mut ScreenForgeApp, ui: &mut egui::Ui) {
    poll(app, ui.ctx());
    if app.uploading {
        // Keep polling for the worker's result while the frame is idle.
        ui.ctx().request_repaint();
        return;
    }
    let Some(url) = app.upload_url.clone() else {
        return;
    };

    // auto_close_upload_window: the share box hides 3 s after it appears, the
    // same delay the C# toast used. Hiding keeps the app alive (tray mode).
    if app.settings.auto_close_upload_window {
        let mut guard = hide_at().lock().unwrap();
        match *guard {
            Some(at) if std::time::Instant::now() >= at => {
                *guard = None;
                app.upload_url = None;
                return;
            }
            Some(_) => ui.ctx().request_repaint(),
            None => *guard = Some(std::time::Instant::now() + std::time::Duration::from_secs(3)),
        }
    }

    ui.add_space(10.0);
    egui::Frame::new()
        .fill(theme::SURFACE)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .inner_margin(egui::Margin::same(10))
        .corner_radius(8.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Paylaşım bağlantısı").strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Kopyala").clicked() {
                        ui.ctx().copy_text(url.clone());
                        app.notify("bağlantı kopyalandı");
                    }
                });
            });
            ui.add_space(4.0);
            egui::ScrollArea::horizontal().show(ui, |ui| {
                ui.add(
                    egui::Label::new(RichText::new(&url).color(theme::ACCENT))
                        .selectable(true),
                );
            });
        });
}

/// Moves a finished upload out of the shared slot into the app state.
fn poll(app: &mut ScreenForgeApp, ctx: &egui::Context) {
    let Some(outcome) = slot().lock().ok().and_then(|mut guard| guard.take()) else {
        return;
    };
    app.uploading = false;
    match outcome {
        Ok(url) => {
            if app.settings.auto_copy_link_after_upload {
                ctx.copy_text(url.clone());
            }
            app.upload_url = Some(url);
            app.status = "yüklendi".to_string();
            app.notify("yükleme tamamlandı");
        }
        Err(e) => {
            app.status = "hazır".to_string();
            app.notify(format!("yükleme hatası: {e}"));
        }
    }
}
