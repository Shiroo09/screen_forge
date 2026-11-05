//! Settings panel: general, output, GIF, video and read-only hotkey overview.
use crate::app::ScreenForgeApp;
use crate::theme;
use sf_settings::{ImageFormat, VideoQuality};

pub fn ui(app: &mut ScreenForgeApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        section(ui, "Genel", |ui| {
            ui.checkbox(&mut app.settings.show_cursor, "İmleci göster");
            ui.checkbox(&mut app.settings.launch_at_startup, "Windows açılışında başlat");
            ui.checkbox(
                &mut app.settings.auto_copy_link_after_upload,
                "Yükleme sonrası bağlantıyı kopyala",
            );
            ui.checkbox(
                &mut app.settings.auto_close_upload_window,
                "Yükleme penceresini otomatik kapat",
            );
        });

        section(ui, "Çıktı", |ui| {
            egui::ComboBox::from_label("Biçim")
                .selected_text(format_name(app.settings.output_format))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut app.settings.output_format, ImageFormat::Png, "PNG");
                    ui.selectable_value(&mut app.settings.output_format, ImageFormat::Jpeg, "JPEG");
                    ui.selectable_value(&mut app.settings.output_format, ImageFormat::Webp, "WebP");
                });

            ui.horizontal(|ui| {
                ui.label("Kalite");
                ui.add(egui::Slider::new(&mut app.settings.quality, 1..=100));
            });

            ui.horizontal(|ui| {
                ui.label("Kayıt klasörü");
                let width = (ui.available_width() - 90.0).max(120.0);
                ui.add(egui::TextEdit::singleline(&mut app.settings.save_directory).desired_width(width));
                if ui.button("Gözat").clicked() {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        app.settings.save_directory = dir.to_string_lossy().into_owned();
                    }
                }
            });
        });

        section(ui, "GIF", |ui| {
            ui.checkbox(&mut app.settings.gif.enabled, "GIF kaydı etkin");
            ui.add_enabled_ui(app.settings.gif.enabled, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Kare hızı");
                    ui.add(egui::DragValue::new(&mut app.settings.gif.fps).range(1..=60));
                });

                egui::ComboBox::from_label("Renk sayısı")
                    .selected_text(app.settings.gif.color_count.to_string())
                    .show_ui(ui, |ui| {
                        for n in [64, 128, 256] {
                            ui.selectable_value(&mut app.settings.gif.color_count, n, n.to_string());
                        }
                    });

                egui::ComboBox::from_label("Nicemleyici")
                    .selected_text(app.settings.gif.quantizer.clone())
                    .show_ui(ui, |ui| {
                        for name in ["Neural", "Octree"] {
                            ui.selectable_value(
                                &mut app.settings.gif.quantizer,
                                name.to_string(),
                                name,
                            );
                        }
                    });

                ui.horizontal(|ui| {
                    ui.label("Örnekleme faktörü");
                    ui.add(egui::DragValue::new(&mut app.settings.gif.sampling_factor).range(1..=20));
                });
                // Recording duration lives on the Record panel: it is a
                // per-capture choice, not a persisted preference (the C# settings
                // file has no field for it).

                ui.checkbox(&mut app.settings.gif.dithering, "Titreşim (dithering)");
                ui.checkbox(&mut app.settings.gif.use_global_palette, "Genel palet kullan");
                ui.checkbox(&mut app.settings.gif.optimize_unchanged_pixels, "Değişmeyen pikselleri atla");
                ui.horizontal(|ui| {
                    ui.label("Değişim toleransı");
                    ui.add(egui::DragValue::new(&mut app.settings.gif.change_tolerance).range(0..=32));
                });
                ui.horizontal(|ui| {
                    ui.label("Kare belleği üst sınırı (MB)");
                    ui.add(egui::DragValue::new(&mut app.settings.gif.max_frame_memory_mb).range(32..=8192));
                });
                ui.checkbox(&mut app.settings.gif.capture_cursor, "İmleci kayda ekle");
                ui.checkbox(&mut app.settings.gif.highlight_clicks, "Tıklamaları vurgula");
                ui.checkbox(&mut app.settings.gif.highlight_cursor, "İmleç halkası göster");
                ui.checkbox(&mut app.settings.gif.show_keys, "Tuş etiketlerini göster");
            });
        });

        section(ui, "Video", |ui| {
            ui.checkbox(&mut app.settings.video.enabled, "Video kaydı etkin");
            ui.add_enabled_ui(app.settings.video.enabled, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Kare hızı");
                    ui.add(egui::DragValue::new(&mut app.settings.video.fps).range(1..=60));
                });

                egui::ComboBox::from_label("Kalite")
                    .selected_text(quality_name(app.settings.video.quality))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut app.settings.video.quality, VideoQuality::Low, "Düşük");
                        ui.selectable_value(
                            &mut app.settings.video.quality,
                            VideoQuality::Medium,
                            "Orta",
                        );
                        ui.selectable_value(&mut app.settings.video.quality, VideoQuality::High, "Yüksek");
                    });

                ui.checkbox(&mut app.settings.video.capture_cursor, "İmleci kayda ekle");
                ui.checkbox(&mut app.settings.video.highlight_clicks, "Tıklamaları vurgula");
                ui.checkbox(&mut app.settings.video.record_system_audio, "Sistem sesini kaydet");
                ui.checkbox(&mut app.settings.video.record_microphone, "Mikrofonu kaydet");
                ui.checkbox(&mut app.settings.video.show_countdown, "Geri sayım göster");
            });
        });

        section(ui, "Kısayollar", |ui| {
            let rows = [
                ("Bölge yakalama", app.settings.region_hotkey.display()),
                ("Tam ekran", app.settings.full_screen_hotkey.display()),
                ("Tam ekran yükleme", app.settings.full_screen_upload_hotkey.display()),
                ("Kolaj", app.settings.collage_hotkey.display()),
                ("Hızlı çeviri", app.settings.quick_translate_hotkey.display()),
            ];
            for (name, value) in rows {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(name).color(theme::TEXT));
                    ui.label(egui::RichText::new(value).color(theme::TEXT_DIM));
                });
            }
            ui.label(
                egui::RichText::new("Kısayollar şimdilik salt okunur.")
                    .color(theme::TEXT_DIM)
                    .size(12.0),
            );
        });

        if ui
            .add(egui::Button::new(egui::RichText::new("Kaydet").size(15.0)))
            .clicked()
        {
            let mut errors = Vec::new();
            if let Err(e) = sf_settings::set_startup_enabled(app.settings.launch_at_startup) {
                errors.push(format!("başlangıç ayarı: {e}"));
            }
            if let Err(e) = app.settings.save(&sf_settings::settings_path()) {
                errors.push(format!("kaydetme: {e}"));
            }
            if errors.is_empty() {
                app.notify("ayarlar kaydedildi");
            } else {
                app.notify(format!("hata — {}", errors.join("; ")));
            }
        }
    });
}

/// A bordered card with an accent heading, matching the rest of the app.
fn section(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(theme::SURFACE)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .inner_margin(egui::Margin::same(14))
        .corner_radius(10.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new(title).size(15.0).strong().color(theme::ACCENT));
            ui.add_space(6.0);
            add(ui);
        });
    ui.add_space(8.0);
}

fn format_name(f: ImageFormat) -> &'static str {
    match f {
        ImageFormat::Png => "PNG",
        ImageFormat::Jpeg => "JPEG",
        ImageFormat::Webp => "WebP",
    }
}

fn quality_name(q: VideoQuality) -> &'static str {
    match q {
        VideoQuality::Low => "Düşük",
        VideoQuality::Medium => "Orta",
        VideoQuality::High => "Yüksek",
    }
}
