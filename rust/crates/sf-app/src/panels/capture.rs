//! Capture panel ("Yakala"): full-screen / region capture, preview, save, clipboard.

use crate::app::{ImageBuf, RecentItem, ScreenForgeApp};
use crate::theme;
use egui::{Color32, RichText};

/// User intent gathered while the UI is borrowed; applied once the borrow ends.
enum Action {
    FullScreen,
    Region,
    Copy,
    Save,
    Presenter,
    Upload,
}

pub fn ui(app: &mut ScreenForgeApp, ui: &mut egui::Ui) {
    let mut action = None;
    let has_image = app.captured.is_some();

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.heading("Ekran Yakala");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(&app.status)
                    .color(theme::TEXT_DIM)
                    .size(12.0),
            );
        });
    });
    ui.label(
        RichText::new("Tam ekranı ya da seçtiğin bölgeyi yakala, kaydet veya panoya kopyala.")
            .color(theme::TEXT_DIM),
    );
    ui.add_space(10.0);

    // --- Mode bar (Bölge · Tam ekran · Serbest), as in the old overlay -------
    egui::Frame::new()
        .fill(theme::SURFACE)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .inner_margin(egui::Margin::symmetric(6, 5))
        .corner_radius(6.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for mode in crate::app::CaptureMode::ALL {
                    let selected = app.capture_mode == mode;
                    let label = RichText::new(mode.title())
                        .size(13.0)
                        .color(if selected { Color32::WHITE } else { theme::TEXT });
                    let build = move || {
                        egui::Button::new(label)
                            .min_size(egui::vec2(96.0, 30.0))
                            .corner_radius(4.0)
                    };
                    let response = if selected {
                        theme::accent_button(ui, build)
                    } else {
                        theme::stateful_button(
                            ui,
                            Color32::TRANSPARENT,
                            theme::SURFACE_HOVER,
                            theme::SURFACE_HOVER,
                            build,
                        )
                    };
                    if response.clicked() {
                        app.capture_mode = mode;
                    }
                }
            });
        });

    ui.add_space(8.0);

    // "Serbest" is not a capture at all: it drops straight into the collage canvas.
    // The action bar and preview below belong to the region/full-screen path only.
    if app.capture_mode == crate::app::CaptureMode::Freeform {
        crate::panels::collage::ui(app, ui);
        return;
    }

    // --- Action buttons -----------------------------------------------------
    egui::Frame::new()
        .fill(theme::SURFACE)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .inner_margin(egui::Margin::same(12))
        .corner_radius(8.0)
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                // The primary action follows the selected mode.
                let primary = match app.capture_mode {
                    crate::app::CaptureMode::Region => "Bölge seç",
                    _ => "Tam ekran yakala",
                };
                if action_button(ui, primary, true).clicked() {
                    action = Some(match app.capture_mode {
                        crate::app::CaptureMode::Region => Action::Region,
                        _ => Action::FullScreen,
                    });
                }
                if action_button(ui, "Panoya kopyala", false).clicked() {
                    action = Some(Action::Copy);
                }
                if action_button(ui, "Kaydet", false).clicked() {
                    action = Some(Action::Save);
                }
                if action_button(ui, "Sunum katmanı", false).clicked() {
                    action = Some(Action::Presenter);
                }
                let uploading = app.uploading;
                let upload_label = if uploading { "Yükleniyor..." } else { "Buluta yükle" };
                ui.add_enabled_ui(!uploading, |ui| {
                    if action_button(ui, upload_label, false).clicked() {
                        action = Some(Action::Upload);
                    }
                });
            });
        });

    ui.add_space(10.0);

    // --- Preview ------------------------------------------------------------
    let max_h = (ui.available_height() - 130.0).clamp(180.0, 460.0);
    egui::Frame::new()
        .fill(theme::SURFACE)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .inner_margin(egui::Margin::same(8))
        .corner_radius(8.0)
        .show(ui, |ui| {
            ui.set_min_height(max_h);
            ui.set_min_width(ui.available_width());
            match &mut app.captured {
                Some(img) => {
                    let tex = img.texture(ui.ctx());
                    let size = fit(
                        img.width as f32,
                        img.height as f32,
                        ui.available_width(),
                        max_h - 24.0,
                    );
                    ui.add(egui::Image::new(&tex).fit_to_exact_size(size));
                    ui.label(
                        RichText::new(format!("{} × {} px", img.width, img.height))
                            .color(theme::TEXT_DIM)
                            .size(12.0),
                    );
                }
                None => {
                    ui.vertical_centered(|ui| {
                        ui.add_space(max_h * 0.32);
                        ui.label(
                            RichText::new("Henüz görüntü yok")
                                .color(theme::TEXT)
                                .size(17.0),
                        );
                        ui.label(
                            RichText::new("Yukarıdan bir yakalama başlat")
                                .color(theme::TEXT_DIM)
                                .size(12.0),
                        );
                    });
                }
            }
        });

    // --- Recent items -------------------------------------------------------
    if !app.recents.is_empty() {
        ui.add_space(10.0);
        ui.label(RichText::new("Son işlemler").strong());
        ui.add_space(4.0);
        for item in app.recents.iter().take(5) {
            egui::Frame::new()
                .fill(theme::SURFACE)
                .inner_margin(egui::Margin::symmetric(10, 6))
                .corner_radius(6.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&item.label).color(theme::TEXT).size(13.0));
                        ui.label(
                            RichText::new(&item.path)
                                .color(theme::TEXT_DIM)
                                .size(11.0),
                        );
                    });
                });
            ui.add_space(3.0);
        }
    }

    if let Some(action) = action {
        run(app, action, has_image);
    }

    // Upload progress / share-link box, fed by the worker thread.
    crate::panels::upload::ui(app, ui);
}

/// Applies the clicked action, with full access to the app state.
fn run(app: &mut ScreenForgeApp, action: Action, has_image: bool) {
    match action {
        Action::FullScreen => match sf_capture::capture_virtual_screen() {
            Ok(c) => set_captured(app, ImageBuf::from_captured(c), "tam ekran yakalandı"),
            Err(e) => app.notify(format!("hata: {e}")),
        },
        Action::Region => match sf_capture::overlay_window::pick_region() {
            Some(rect) => match sf_capture::capture_region(rect.0, rect.1, rect.2, rect.3) {
                Ok(c) => {
                    let label = format!("bölge yakalandı ({}×{})", rect.2, rect.3);
                    set_captured(app, ImageBuf::from_captured(c), &label)
                }
                Err(e) => app.notify(format!("hata: {e}")),
            },
            None => app.notify("bölge seçimi iptal edildi"),
        },
        Action::Copy => {
            let Some(img) = &app.captured else {
                app.notify("önce bir görüntü yakala");
                return;
            };
            match sf_capture::copy_bgra_to_clipboard(img.width, img.height, &img.bgra) {
                Ok(()) => app.notify("panoya kopyalandı"),
                Err(e) => app.notify(format!("hata: {e}")),
            }
        }
        Action::Upload => crate::panels::upload::start(app),
        Action::Presenter => {            app.notify("sunum katmanı açılıyor — Esc ile çık");
            sf_capture::presenter_window::run();
            app.notify("sunum katmanı kapandı");
        }
        Action::Save => {
            if !has_image {
                app.notify("önce bir görüntü yakala");
                return;
            }
            let format = app.settings.output_format;
            let mut dialog =
                rfd::FileDialog::new().set_file_name(format!("ScreenForge.{}", ext_of(format)));
            if !app.settings.save_directory.trim().is_empty() {
                dialog = dialog.set_directory(&app.settings.save_directory);
            }
            let Some(path) = dialog.save_file() else {
                return;
            };
            let Some(img) = &app.captured else {
                return;
            };
            match save_image(&path, img.width, img.height, &img.bgra, format, app.settings.quality) {
                Ok(()) => {
                    let shown = path.display().to_string();
                    app.recents.insert(
                        0,
                        RecentItem {
                            label: format!("Kaydedildi · {}×{}", img.width, img.height),
                            path: shown.clone(),
                        },
                    );
                    app.recents.truncate(8);
                    app.status = format!("kaydedildi: {shown}");
                    app.notify(format!("kaydedildi: {shown}"));
                }
                Err(e) => app.notify(format!("hata: {e}")),
            }
        }
    }
}

fn set_captured(app: &mut ScreenForgeApp, img: ImageBuf, message: &str) {
    app.status = format!("yakalandı: {}×{}", img.width, img.height);
    app.captured = Some(img);
    // A new capture invalidates the previous capture's share link.
    app.upload_url = None;
    app.notify(message);
}

fn action_button(ui: &mut egui::Ui, label: &str, primary: bool) -> egui::Response {
    let label = label.to_string();
    let build = move || {
        egui::Button::new(
            RichText::new(label)
                .size(15.0)
                .strong()
                .color(if primary { Color32::WHITE } else { theme::TEXT }),
        )
        .corner_radius(8.0)
        .min_size(egui::vec2(168.0, 44.0))
    };
    // Colours come from the state ramp, not `.fill()`, so hover/pressed react.
    if primary {
        theme::accent_button(ui, build)
    } else {
        theme::surface_button(ui, build)
    }
}

/// Scales `w`×`h` to fit the box without cropping or upscaling past 1:1.
fn fit(w: f32, h: f32, max_w: f32, max_h: f32) -> egui::Vec2 {
    if w <= 0.0 || h <= 0.0 || max_w <= 0.0 || max_h <= 0.0 {
        return egui::vec2(1.0, 1.0);
    }
    let s = (max_w / w).min(max_h / h).min(1.0);
    egui::vec2(w * s, h * s)
}

fn ext_of(format: sf_settings::ImageFormat) -> &'static str {
    match format {
        sf_settings::ImageFormat::Png => "png",
        sf_settings::ImageFormat::Jpeg => "jpg",
        sf_settings::ImageFormat::Webp => "webp",
    }
}

/// Encodes a BGRA buffer to the chosen format, swapping B/R to RGBA on the way.
fn save_image(
    path: &std::path::Path,
    width: i32,
    height: i32,
    bgra: &[u8],
    format: sf_settings::ImageFormat,
    quality: i32,
) -> Result<(), String> {
    if width <= 0 || height <= 0 || bgra.len() < (width * height * 4) as usize {
        return Err("geçersiz görüntü tamponu".to_string());
    }

    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for px in bgra.chunks_exact(4) {
        rgba.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
    }
    let img = image::RgbaImage::from_raw(width as u32, height as u32, rgba)
        .ok_or_else(|| "görüntü boyutu uyuşmuyor".to_string())?;

    if format == sf_settings::ImageFormat::Jpeg {
        let file =
            std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let rgb = image::DynamicImage::ImageRgba8(img).to_rgb8();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
            file,
            quality.clamp(1, 100) as u8,
        );
        encoder
            .encode_image(&rgb)
            .map_err(|e| format!("jpeg: {e}"))?;
        return Ok(());
    }

    let ifmt = match format {
        sf_settings::ImageFormat::Png => image::ImageFormat::Png,
        sf_settings::ImageFormat::Jpeg => unreachable!(),
        sf_settings::ImageFormat::Webp => image::ImageFormat::WebP,
    };
    img.save_with_format(path, ifmt)
        .map_err(|e| format!("kaydetme: {e}"))
}
