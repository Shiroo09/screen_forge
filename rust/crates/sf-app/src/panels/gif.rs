//! GIF editörü: GIF aç, kare işlemlerini uygula (azalt / ters çevir), yeniden kodla.
//!
//! Çözme ve kodlama `sf-cli/src/gifedit.rs`'teki çalışan akışın aynısı; kare
//! işlemleri ortak `sf_edit::frame_ops` üzerinden yapılır. Yüklenen kareler
//! `app.rs`'e alan eklemeden modül düzeyinde tutulur; önizleme dokusu oturum
//! kimliği değişince yeniden kurulur.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use sf_edit::frame_ops::{self, DelayMergeMode};
use sf_edit::EditorFrame;
use sf_gif::gif_file::{GifFile, Rect};

use crate::app::ScreenForgeApp;
use crate::theme;

/// Çözülmüş GIF: tuval boyutu + her karenin BGRA'sı ve gecikmesi (ms).
struct LoadedGif {
    width: i32,
    height: i32,
    frames: Vec<(Vec<u8>, i32)>,
}

/// Seçilen (henüz açılmamış) dosya yolu.
fn selection() -> &'static Mutex<Option<String>> {
    static S: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(None))
}

/// Açık GIF oturumu.
fn session() -> &'static Mutex<Option<LoadedGif>> {
    static S: OnceLock<Mutex<Option<LoadedGif>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(None))
}

/// Önizlemesi kurulmuş oturumun kimliği.
fn session_id() -> &'static AtomicU64 {
    static S: AtomicU64 = AtomicU64::new(0);
    &S
}

/// Önizleme dokusu ve hangi oturuma ait olduğu.
fn preview_state() -> &'static Mutex<(u64, Option<egui::TextureHandle>)> {
    static S: OnceLock<Mutex<(u64, Option<egui::TextureHandle>)>> = OnceLock::new();
    S.get_or_init(|| Mutex::new((0, None)))
}

/// Oturumu tazeler: önizlemeyi geçersiz kılar ve kimliği artırır.
fn bump_session(app: &mut ScreenForgeApp, count: usize) {
    *preview_state().lock().unwrap() = (0, None);
    session_id().fetch_add(1, Ordering::Relaxed);
    if let Some(g) = session().lock().unwrap().as_ref() {
        app.gif_info = Some((g.width, g.height, count));
    }
}

pub fn ui(app: &mut ScreenForgeApp, ui: &mut egui::Ui) {
    ui.heading("GIF Düzenle");
    ui.label(
        egui::RichText::new("Kareleri azalt, sırayı çevir, yeniden kodla.")
            .color(theme::TEXT_DIM)
            .size(12.0),
    );
    ui.add_space(8.0);

    let mut pick = false;
    card(ui, |ui| {
        ui.horizontal(|ui| {
            if ui.button("GIF Aç…").clicked() {
                pick = true;
            }
            let label = selection()
                .lock()
                .unwrap()
                .as_ref()
                .map(|p| {
                    std::path::Path::new(p)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| p.clone())
                })
                .unwrap_or_else(|| "Henüz dosya seçilmedi".to_string());
            ui.label(egui::RichText::new(label).color(theme::TEXT));
        });
    });

    let Some((w, h, count)) = app.gif_info else {
        if pick {
            open_gif(app);
        }
        ui.label(
            egui::RichText::new("Önizleme için bir GIF açın.")
                .color(theme::TEXT_DIM)
                .size(12.0),
        );
        return;
    };

    // ── Kare bilgisi + önizleme ──────────────────────────────────────────────
    card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Kare Bilgisi").color(theme::ACCENT).strong());
            ui.label(egui::RichText::new(format!("{w} × {h}  ·  {count} kare")).color(theme::TEXT));
        });
        ui.add_space(6.0);
        let max = ui.available_width().min(560.0);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(max, max * 0.5625), egui::Sense::hover());
        ui.painter().rect_filled(rect, 6.0, theme::BG);
        match preview_texture(ui.ctx()) {
            Some(tex) => {
                let size = tex.size_vec2();
                let s = (rect.width() / size.x).min(rect.height() / size.y);
                let shown = egui::Rect::from_center_size(rect.center(), size * s);
                ui.painter().image(
                    tex.id(),
                    shown,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
                ui.label(
                    egui::RichText::new("Önizleme: ilk kare")
                        .color(theme::TEXT_DIM)
                        .size(11.0),
                );
            }
            None => {
                ui.label(egui::RichText::new("Önizleme hazırlanıyor…").color(theme::TEXT_DIM));
            }
        }
    });

    // ── İşlemler ────────────────────────────────────────────────────────────
    let mut apply = false;
    let mut first = false;
    let mut save = false;
    card(ui, |ui| {
        ui.label(egui::RichText::new("İşlemler").color(theme::ACCENT).strong());
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Koru:");
            ui.add(egui::DragValue::new(&mut app.gif_keep).range(1..=100).speed(1.0));
            ui.label("Atla:");
            ui.add(egui::DragValue::new(&mut app.gif_remove).range(1..=100).speed(1.0));
            apply = ui.button("Kare azalt").clicked();
        });
        ui.add_space(4.0);
        ui.checkbox(&mut app.gif_reverse, "Ters çevir");
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            first = ui.button("İlk kareyi göster").clicked();
            save = ui.button("Kaydet…").clicked();
        });
    });

    if pick {
        open_gif(app);
    }
    if apply {
        apply_reduce(app);
    }
    if first {
        bump_session(app, count);
        app.notify("İlk kare önizlemede");
    }
    if save {
        save_gif(app);
    }
}

fn card(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(theme::SURFACE)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .inner_margin(egui::Margin::same(12))
        .corner_radius(8.0)
        .show(ui, content);
    ui.add_space(8.0);
}

/// GIF'i diskten çözer, panel oturumunu ve `app.gif_info`'yu tazeler.
fn open_gif(app: &mut ScreenForgeApp) {
    let Some(path) = rfd::FileDialog::new().add_filter("GIF", &["gif"]).pick_file() else {
        return;
    };
    let path = path.to_string_lossy().to_string();

    match decode(&path) {
        Ok(g) => {
            let count = g.frames.len();
            *selection().lock().unwrap() = Some(path.clone());
            *session().lock().unwrap() = Some(g);
            bump_session(app, count);
            app.status = format!("GIF: {path}");
            app.notify(format!("GIF açıldı: {count} kare"));
        }
        Err(e) => {
            app.gif_info = None;
            app.notify(format!("GIF açılamadı: {e}"));
        }
    }
}

/// "Kare azalt": keep/remove kuralının sonucunu önizleme için hesaplar.
/// Kaydetme aynı işlemi kaynak dosyadan yeniden uygular.
fn apply_reduce(app: &mut ScreenForgeApp) {
    let keep = app.gif_keep.max(1) as usize;
    let remove = app.gif_remove.max(1) as usize;
    let (before, after) = {
        let guard = session().lock().unwrap();
        let Some(g) = guard.as_ref() else {
            drop(guard);
            app.notify("Önce bir GIF açın");
            return;
        };
        let edit = to_editor_frames(g);
        let after = frame_ops::reduce(&edit, keep, remove, DelayMergeMode::Distribute).len();
        (g.frames.len(), after)
    };
    bump_session(app, after);
    app.notify(format!("Kare azalt: {before} → {after}"));
}

/// İşlenmiş GIF'i seçilen yola yazar.
fn save_gif(app: &mut ScreenForgeApp) {
    let Some(src) = selection().lock().unwrap().clone() else {
        app.notify("Önce bir GIF açın");
        return;
    };
    let mut dialog = rfd::FileDialog::new().add_filter("GIF", &["gif"]);
    if let Some(name) = std::path::Path::new(&src).file_name() {
        dialog = dialog.set_file_name(name.to_string_lossy());
    }
    let Some(out) = dialog.save_file() else {
        return;
    };
    let out = out.to_string_lossy().to_string();

    match encode(&src, &out, app.gif_keep.max(1) as usize, app.gif_remove.max(1) as usize, app.gif_reverse)
    {
        Ok(n) => {
            app.status = format!("GIF kaydedildi: {out}");
            app.notify(format!("Kaydedildi ({n} kare): {out}"));
        }
        Err(e) => app.notify(format!("Kaydedilemedi: {e}")),
    }
}

/// Önizleme dokusunu (gerekirse yeniden) kurar.
fn preview_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let sid = session_id().load(Ordering::Relaxed);
    let mut state = preview_state().lock().unwrap();
    if state.0 != sid {
        state.0 = sid;
        state.1 = None;
    }
    if state.1.is_none() {
        let guard = session().lock().unwrap();
        let (w, h, bgra) = {
            let g = guard.as_ref()?;
            let (bgra, _) = g.frames.first()?;
            (g.width, g.height, bgra.clone())
        };
        drop(guard);
        let mut rgba = Vec::with_capacity(bgra.len());
        for px in bgra.chunks_exact(4) {
            rgba.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
        }
        let img = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
        state.1 = Some(ctx.load_texture("gif-preview", img, egui::TextureOptions::LINEAR));
    }
    state.1.clone()
}

/// GIF'i tuval üzerine bindirilmiş BGRA kareler + gecikmeler olarak çözer.
fn decode(path: &str) -> Result<LoadedGif, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("oku {path}: {e}"))?;
    let mut opts = gif::DecodeOptions::new();
    opts.set_color_output(gif::ColorOutput::RGBA);
    let mut reader = opts
        .read_info(std::io::Cursor::new(&bytes))
        .map_err(|e| format!("gif çöz: {e}"))?;

    let width = reader.width() as i32;
    let height = reader.height() as i32;
    let mut canvas = vec![0u8; (width * height * 4) as usize];
    let mut frames = Vec::new();

    while let Some(frame) = reader.read_next_frame().map_err(|e| format!("kare: {e}"))? {
        let fx = frame.left as i32;
        let fy = frame.top as i32;
        let fw = frame.width as i32;
        let fh = frame.height as i32;

        if frame.dispose == gif::DisposalMethod::Background {
            canvas.fill(0);
        }
        for y in 0..fh {
            for x in 0..fw {
                let src = ((y * fw + x) * 4) as usize;
                let dx = fx + x;
                let dy = fy + y;
                if dx < 0 || dy < 0 || dx >= width || dy >= height {
                    continue;
                }
                let dst = ((dy * width + dx) * 4) as usize;
                canvas[dst] = frame.buffer[src + 2]; // R→B
                canvas[dst + 1] = frame.buffer[src + 1];
                canvas[dst + 2] = frame.buffer[src];
                canvas[dst + 3] = frame.buffer[src + 3];
            }
        }
        frames.push((canvas.clone(), frame.delay as i32 * 10)); // 1/100 s → ms
    }

    if frames.is_empty() {
        return Err("GIF'te kare yok".to_string());
    }
    Ok(LoadedGif {
        width,
        height,
        frames,
    })
}

fn to_editor_frames(g: &LoadedGif) -> Vec<Arc<EditorFrame>> {
    g.frames
        .iter()
        .map(|(bgra, delay)| EditorFrame::new(bgra.clone(), *delay, Default::default()))
        .collect()
}

/// Kaynak dosyayı çözer, işlemleri uygular ve yeniden kodlar.
fn encode(
    path: &str,
    out: &str,
    keep: usize,
    remove: usize,
    reverse: bool,
) -> Result<usize, String> {
    let g = decode(path)?;
    let mut edit = to_editor_frames(&g);
    edit = frame_ops::reduce(&edit, keep.max(1), remove.max(1), DelayMergeMode::Distribute);
    if reverse {
        edit = frame_ops::reverse(&edit);
    }

    let file = std::fs::File::create(out).map_err(|e| format!("oluştur {out}: {e}"))?;
    let mut gif = GifFile::new(std::io::BufWriter::new(file));
    gif.set_canvas_size(g.width, g.height);
    gif.maximum_number_color = 256;
    gif.quantizer_type = sf_gif::QuantizerType::Octree;

    for frame in &edit {
        gif.add_frame(
            &frame.pixels_uncached(),
            Rect {
                x: 0,
                y: 0,
                width: g.width,
                height: g.height,
            },
            frame.delay,
            false,
        );
    }
    gif.finish();
    Ok(edit.len())
}
