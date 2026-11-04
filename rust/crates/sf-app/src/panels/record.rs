//! Record panel: capture the screen to GIF or MP4 from the desktop UI.
//!
//! The capture loop runs on a worker thread so the frame never blocks. `app` has
//! no channel field, so the worker reports back through the module-level
//! `RESULT` slot, which `ui()` drains on every pass.
//!
//! shortcut: the result is only drained while the Kayıt tab is on screen; a job
//! that finishes on another tab stays "kaydediliyor" until the user returns.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::app::ScreenForgeApp;
use crate::theme;

/// Set by the "İptal" button; polled once per captured frame.
static CANCEL: OnceLock<Arc<AtomicBool>> = OnceLock::new();
/// Final message from the worker, drained by `ui()`.
static RESULT: OnceLock<Arc<Mutex<Option<String>>>> = OnceLock::new();

fn cancel_flag() -> Arc<AtomicBool> {
    CANCEL.get_or_init(|| Arc::new(AtomicBool::new(false))).clone()
}

fn result_slot() -> Arc<Mutex<Option<String>>> {
    RESULT.get_or_init(|| Arc::new(Mutex::new(None))).clone()
}

pub fn ui(app: &mut ScreenForgeApp, ui: &mut egui::Ui) {
    // The worker thread cannot touch `app`; it leaves its final message here.
    if let Some(msg) = result_slot().lock().unwrap().take() {
        app.recording = false;
        app.status = "hazır".to_string();
        app.notify(msg);
    }

    ui.heading("Kayıt");
    ui.label(
        egui::RichText::new("Ekranı GIF veya MP4 olarak kaydedin")
            .color(theme::TEXT_DIM),
    );
    ui.add_space(10.0);

    let recording = app.recording;

    if recording {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("●").color(theme::ACCENT).size(18.0));
            ui.label(egui::RichText::new("kaydediliyor...").color(theme::ACCENT).strong());
            if ui.button("İptal").clicked() {
                cancel_flag().store(true, Ordering::SeqCst);
                app.notify("kayıt iptal ediliyor...");
            }
        });
        // Keep the frame ticking so the cancel button stays responsive.
        ui.ctx().request_repaint();
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
                    ui.label("Süre");
                    ui.add(
                        egui::DragValue::new(&mut app.record_seconds)
                            .range(1.0..=120.0)
                            .speed(0.5)
                            .suffix(" sn"),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("FPS");
                    ui.add(
                        egui::DragValue::new(&mut app.record_fps)
                            .range(1..=30)
                            .suffix(" fps"),
                    );
                });

                ui.add_space(10.0);
                save_clicked = ui
                    .add_sized(
                        [160.0, 34.0],
                        egui::Button::new(egui::RichText::new("Kaydet").strong()),
                    )
                    .clicked();
            });

        if !app.record_gif && !sf_record::is_supported() {
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

/// Picks the output path and hands the capture loop to a worker thread.
fn start(app: &mut ScreenForgeApp) {
    let gif = app.record_gif;
    if !gif && !sf_record::is_supported() {
        app.notify("MP4 kaydı bu platformda desteklenmiyor");
        return;
    }

    let seconds = app.record_seconds.clamp(1.0, 120.0);
    let fps = app.record_fps.clamp(1, 30);
    let ext = if gif { "gif" } else { "mp4" };

    let Some(path) = rfd::FileDialog::new()
        .set_file_name(format!("screenforge-kayit.{ext}"))
        .add_filter(ext.to_uppercase(), &[ext])
        .save_file()
    else {
        return;
    };
    let path = path.to_string_lossy().into_owned();

    let cancel = cancel_flag();
    cancel.store(false, Ordering::SeqCst);
    let slot = result_slot();
    *slot.lock().unwrap() = None;

    app.recording = true;
    app.status = "kaydediliyor...".to_string();

    std::thread::spawn(move || {
        let outcome = if gif {
            record_gif(&path, seconds, fps, &cancel)
        } else {
            record_mp4(&path, seconds, fps, &cancel)
        };
        let msg = match outcome {
            Ok(n) if cancel.load(Ordering::SeqCst) => format!("kayıt iptal edildi ({n} kare)"),
            Ok(n) => format!("{n} kare kaydedildi → {path}"),
            Err(e) => format!("kayıt hatası: {e}"),
        };
        *slot.lock().unwrap() = Some(msg);
    });
}

/// Captures `seconds` × `fps` frames of the virtual screen into an animated GIF.
fn record_gif(path: &str, seconds: f32, fps: i32, cancel: &AtomicBool) -> Result<usize, String> {
    use sf_gif::gif_file::{GifFile, Rect};

    let fps = fps.clamp(1, 30);
    let delay_ms = (1000.0 / fps as f64).round().max(1.0) as i32;
    let total = (seconds.max(0.1) as f64 * fps as f64).round() as usize;

    let first = sf_capture::capture_virtual_screen()?;
    let (w, h) = (first.width, first.height);
    if w <= 0 || h <= 0 {
        return Err("ekran yakalanamadı".to_string());
    }

    let file = std::fs::File::create(path).map_err(|e| format!("{path}: {e}"))?;
    // Buffered writer: frames stream to disk instead of accumulating in memory.
    let mut gif = GifFile::new(std::io::BufWriter::new(file));
    gif.set_canvas_size(w, h);

    let rect = Rect { x: 0, y: 0, width: w, height: h };
    let interval = std::time::Duration::from_millis(delay_ms as u64);
    let mut frames = 0usize;
    let mut next = Some(first);

    for _ in 0..total {
        let started = std::time::Instant::now();
        let shot = match next.take() {
            Some(f) => f,
            None => sf_capture::capture_virtual_screen()?,
        };
        gif.add_frame(&shot.bgra, rect, delay_ms, false);
        frames += 1;

        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let spent = started.elapsed();
        if spent < interval {
            std::thread::sleep(interval - spent);
        }
    }
    gif.finish();
    Ok(frames)
}

/// Captures `seconds` × `fps` frames of the virtual screen into an H.264 MP4.
fn record_mp4(path: &str, seconds: f32, fps: i32, cancel: &AtomicBool) -> Result<usize, String> {
    let fps = fps.clamp(1, 30);
    let total = (seconds.max(0.1) as f64 * fps as f64).round() as usize;

    let first = sf_capture::capture_virtual_screen()?;
    // H.264 wants even dimensions; cap the long edge (2560) so a 4K desktop
    // does not encode at full size and crawl.
    let (w, h) = sf_media::cap_long_edge(first.width, first.height, sf_media::MAX_LONG_EDGE);
    if w == 0 || h == 0 {
        return Err("ekran yakalanamadı".to_string());
    }

    let bitrate = sf_media::bits_per_second(sf_settings::VideoQuality::High, w, h);
    let mut writer = sf_record::Mp4Writer::new(path, w, h, fps, bitrate)?;
    let frame_dur = writer.frame_duration();
    let interval = std::time::Duration::from_millis((1000 / fps) as u64);

    let mut frames = 0usize;
    let mut next = Some(first);

    for i in 0..total {
        let started = std::time::Instant::now();
        let shot = match next.take() {
            Some(f) => f,
            None => sf_capture::capture_virtual_screen()?,
        };

        let pixels = if shot.width == w && shot.height == h {
            shot.bgra
        } else {
            sf_edit::image_ops::resize_pixels(&shot.bgra, shot.width, shot.height, w, h)
        };
        writer.write_bgra(&pixels, i as i64 * frame_dur)?;
        frames += 1;

        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let spent = started.elapsed();
        if spent < interval {
            std::thread::sleep(interval - spent);
        }
    }

    writer.finish()?;
    Ok(frames)
}
