//! Record panel: capture a chosen screen region to GIF or MP4.
//!
//! The capture loop runs on a worker thread so the frame never blocks; the
//! thread shares its pause/restart/stop state with the status overlay through a
//! single [`sf_record::PauseHandle`]. The final message comes back through the
//! module-level `RESULT` slot, which `ui()` drains on every pass.
//!
//! Region, fps and the encoder settings are read from `sf-settings`; only the
//! per-run duration stays panel state.

use std::sync::{Arc, Mutex, OnceLock};

use sf_record::PauseHandle;

use crate::app::ScreenForgeApp;
use crate::panels::record_worker::{record_gif, record_mp4, GifEncode, RecordingOptions, ScreenSource};
use crate::theme;

/// The live recording, if any. Shared with the status overlay.
static ACTIVE: OnceLock<Arc<Mutex<Option<Active>>>> = OnceLock::new();
/// Final message from the worker, drained by `ui()`.
static RESULT: OnceLock<Arc<Mutex<Option<String>>>> = OnceLock::new();

struct Active {
    handle: PauseHandle,
    region: (i32, i32, i32, i32),
    gif: bool,
}

fn active_slot() -> Arc<Mutex<Option<Active>>> {
    ACTIVE.get_or_init(|| Arc::new(Mutex::new(None))).clone()
}

fn result_slot() -> Arc<Mutex<Option<String>>> {
    RESULT.get_or_init(|| Arc::new(Mutex::new(None))).clone()
}

pub fn ui(app: &mut ScreenForgeApp, ui: &mut egui::Ui) {
    if app.recording {
        // The worker cannot touch `app`; it leaves its final message here.
        if let Some(msg) = result_slot().lock().unwrap().take() {
            app.recording = false;
            app.status = "hazır".to_string();
            *active_slot().lock().unwrap() = None;
            app.notify(msg);
        }
    }

    ui.heading("Kayıt");
    ui.label(
        egui::RichText::new("Ekranın seçtiğiniz bölgesini GIF veya MP4 olarak kaydedin")
            .color(theme::TEXT_DIM),
    );
    ui.add_space(10.0);

    let recording = app.recording;
    let gif = app.record_gif;

    if recording {
        // Clone the handle out of the lock before drawing, so the closure never
        // holds the mutex across a borrow of `app`.
        let handle = active_slot()
            .lock()
            .unwrap()
            .as_ref()
            .map(|a| a.handle.clone());
        recording_bar(app, ui, handle);
        ui.add_space(10.0);
    }

    let mut save_clicked = false;
    ui.add_enabled_ui(!recording, |ui| {
        egui::Frame::new()
            .fill(theme::SURFACE)
            .stroke(egui::Stroke::new(1.0, theme::BORDER))
            .corner_radius(10.0)
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Biçim").strong());
                ui.horizontal(|ui| {
                    if ui.selectable_label(app.record_gif, "GIF").clicked() {
                        app.record_gif = true;
                    }
                    if ui.selectable_label(!app.record_gif, "MP4").clicked() {
                        app.record_gif = false;
                    }
                });

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label("Bölge");
                    let label = match app.record_region {
                        Some((_, _, w, h)) => format!("seçili — {w}×{h}"),
                        None => "tüm ekran".to_string(),
                    };
                    ui.label(egui::RichText::new(label).color(theme::TEXT_DIM));
                    if ui.button("Bölge seç").clicked() {
                        app.record_region = sf_capture::overlay_window::pick_region();
                    }
                    if app.record_region.is_some() && ui.button("Temizle").clicked() {
                        app.record_region = None;
                    }
                });

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Süre");
                    ui.add(
                        egui::DragValue::new(&mut app.record_seconds)
                            .range(1.0..=600.0)
                            .speed(0.5)
                            .suffix(" sn"),
                    );
                });

                ui.add_space(8.0);
                save_clicked = ui
                    .add_sized(
                        [160.0, 34.0],
                        egui::Button::new(egui::RichText::new("Kaydet").strong()),
                    )
                    .clicked();
            });

        // Recording settings live on the Settings tab; surface the active ones
        // here so it is obvious what a run will use.
        ui.add_space(6.0);
        let fps = if gif { app.settings.gif.fps } else { app.settings.video.fps };
        ui.label(
            egui::RichText::new(format!(
                "{fps} fps · {}{}",
                if gif { "GIF" } else { "MP4" },
                if gif && !app.settings.gif.enabled {
                    " · GIF kaydı ayarlarda kapalı"
                } else if !gif && !app.settings.video.enabled {
                    " · video kaydı ayarlarda kapalı"
                } else {
                    ""
                }
            ))
            .color(theme::TEXT_DIM)
            .size(12.0),
        );

        if !gif && !sf_record::is_supported() {
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new("MP4 kaydı bu platformda desteklenmiyor")
                    .color(theme::TEXT_DIM)
                    .size(12.0),
            );
        }
    });

    if save_clicked {
        start(app);
    }
}

/// The "kaydediliyor" bar with pause/resume/restart/stop and the live counters.
fn recording_bar(app: &mut ScreenForgeApp, ui: &mut egui::Ui, handle: Option<PauseHandle>) {
    let paused = handle.as_ref().is_some_and(|h| h.is_paused());
    ui.horizontal(|ui| {
        let (glyph, label, color) = if paused {
            ("||", "duraklatıldı", theme::TEXT_DIM)
        } else {
            ("●", "kaydediliyor...", theme::ACCENT)
        };
        ui.label(egui::RichText::new(glyph).color(color).size(18.0));
        ui.label(egui::RichText::new(label).color(color).strong());

        let Some(handle) = handle else {
            return;
        };

        if ui.button(if paused { "Devam et" } else { "Duraklat" }).clicked() {
            if paused {
                handle.resume();
            } else {
                handle.pause();
            }
        }
        if ui.button("Yeniden başlat").clicked() {
            handle.restart();
            app.notify("kayıt yeniden başlatıldı");
        }
        if ui.button("İptal").clicked() {
            handle.cancel();
            app.notify("kayıt iptal ediliyor...");
        }

        let session = handle.session();
        let elapsed = session.elapsed_ms() / 1000;
        ui.label(
            egui::RichText::new(format!(
                "{:02}:{:02} · {} kare · {:.1} MB",
                elapsed / 60,
                elapsed % 60,
                session.frames(),
                session.bytes() as f64 / (1024.0 * 1024.0)
            ))
            .color(theme::TEXT_DIM),
        );
    });

    // Keep the frame ticking so pause/stop stay responsive.
    ui.ctx().request_repaint();
}

/// Picks the output path and hands the capture loop to a worker thread.
fn start(app: &mut ScreenForgeApp) {
    let gif = app.record_gif;
    if !gif && !sf_record::is_supported() {
        app.notify("MP4 kaydı bu platformda desteklenmiyor");
        return;
    }

    // Only enabled formats may start; `enabled` is a real setting, not a label.
    if gif && !app.settings.gif.enabled {
        app.notify("GIF kaydı ayarlarda kapalı");
        return;
    }
    if !gif && !app.settings.video.enabled {
        app.notify("video kaydı ayarlarda kapalı");
        return;
    }

    // Region: the user's selection, or the whole virtual screen when none.
    let (x, y, w, h) = match app.record_region {
        Some(rect) => rect,
        None => sf_capture::virtual_screen_bounds(),
    };
    if w <= 0 || h <= 0 {
        app.notify("geçerli bir kayıt bölgesi yok");
        return;
    }

    let fps = if gif { app.settings.gif.fps } else { app.settings.video.fps }.clamp(1, 60);
    let seconds = app.record_seconds.clamp(1.0, 600.0);
    let ext = if gif { "gif" } else { "mp4" };

    // Save into the configured directory by default.
    let mut dialog = rfd::FileDialog::new()
        .set_file_name(format!("screenforge-kayit.{ext}"))
        .add_filter(ext.to_uppercase(), &[ext]);
    if !app.settings.save_directory.trim().is_empty() {
        dialog = dialog.set_directory(&app.settings.save_directory);
    }
    let Some(path) = dialog.save_file() else {
        return;
    };
    let path = path.to_string_lossy().into_owned();

    let handle = PauseHandle::new();
    let worker = handle.clone();
    *active_slot().lock().unwrap() = Some(Active { handle: handle.clone(), region: (x, y, w, h), gif });

    let cursor = app.settings.gif.capture_cursor || app.settings.video.capture_cursor;
    let opts = RecordingOptions {
        x,
        y,
        w,
        h,
        fps,
        max_frames: gif.then(|| (seconds as f64 * fps as f64).round().max(1.0) as u64),
        max_bytes: gif.then(|| app.settings.gif.max_frame_memory_mb.max(32) as u64 * 1024 * 1024),
        capture_cursor: cursor,
        system_audio: !gif && app.settings.video.record_system_audio,
        microphone: !gif && app.settings.video.record_microphone,
    };
    let encode = GifEncode {
        color_count: app.settings.gif.color_count,
        quantizer: app.settings.gif.quantizer.clone(),
        sampling_factor: app.settings.gif.sampling_factor,
        dithering: app.settings.gif.dithering,
        use_global_palette: app.settings.gif.use_global_palette,
    };
    let quality = app.settings.video.quality;

    app.recording = true;
    app.status = "kaydediliyor...".to_string();

    std::thread::spawn(move || {
        let outcome = if gif {
            record_gif(&path, &opts, &encode, &worker, &ScreenSource)
        } else {
            record_mp4(&path, &opts, quality, &worker, &ScreenSource)
        };
        let cancelled = worker.is_cancelled();
        let msg = match outcome {
            Ok(frames) if cancelled => format!("kayıt iptal edildi ({frames} kare)"),
            Ok(frames) => format!("{frames} kare kaydedildi → {path}"),
            Err(e) => format!("kayıt hatası: {e}"),
        };
        *result_slot().lock().unwrap() = Some(msg);
    });
}

/// Painted every pass while a recording runs; see `record_overlay`.
pub fn overlay_spec() -> Option<crate::panels::record_overlay::OverlaySpec> {
    let slot = active_slot();
    let active = slot.lock().unwrap();
    let (region, handle, gif) = {
        let active = active.as_ref()?;
        (active.region, active.handle.clone(), active.gif)
    };
    let (vx, vy, _, _) = sf_capture::virtual_screen_bounds();
    Some(crate::panels::record_overlay::OverlaySpec {
        region,
        origin: (vx, vy),
        handle,
        gif,
    })
}

/// Clears the "recording" flag when the worker already reported back but the
/// panel was not on screen to drain it.
pub fn sync(app: &mut ScreenForgeApp) {
    if !app.recording {
        return;
    }
    if let Some(msg) = result_slot().lock().unwrap().take() {
        app.recording = false;
        app.status = "hazır".to_string();
        *active_slot().lock().unwrap() = None;
        app.notify(msg);
    }
}
