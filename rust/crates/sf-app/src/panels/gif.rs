//! GIF editörü: GIF aç, kare işlemleri uygula (azalt / ters çevir), yeniden kodla.
//!
//! Çözme/kodlama mantığı `sf-cli/src/gifedit.rs`'teki çalışan sürümün aynısı.
//! Yüklenen kareler panel durumunda değil, modül düzeyinde tek bir oturumda
//! tutulur; oturum sayısı `GIF_SESSION` ile artırılır ve eski doku önbelleği
//! böylece geçersizleşir.

use std::sync::{Mutex, OnceLock};

use sf_edit::frame_ops::{self, DelayMergeMode};
use sf_edit::EditorFrame;
use sf_gif::gif_file::{GifFile, Rect};

use crate::app::ScreenForgeApp;
use crate::theme;

/// Çözülmüş bir GIF: tuval boyutu + her karenin BGRA'sı ve gecikmesi.
struct LoadedGif {
    path: String,
    width: i32,
    height: i32,
    frames: Vec<(Vec<u8>, i32)>,
}

fn session() -> &'static Mutex<Option<LoadedGif>> {
    static S: OnceLock<Mutex<Option<LoadedGif>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(None))
}

/// Önizlemesi kurulmuş oturumun kimliği; yeni GIF açıldığında artar.
fn session_id() -> &'static std::sync::atomic::AtomicU64 {
    static S: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    &S
}

pub fn ui(app: &mut ScreenForgeApp, ui: &mut egui::Ui) {
    // Seçilen dosya yolu dışında panel durumu tutulmaz; `app.rs` sadece
    // gif_info/keep/remove/reverse alanlarını taşır.
    let mut selected = selection().lock().unwrap().clone();
    let preview_ready = app.gif_info.is_some() && preview_state().lock().unwrap().0 == session_id().load(std::sync::atomic::Ordering::Relaxed);

    let mut open_clicked = false;
    let mut apply_clicked = false;
    let mut save_clicked = false;
    let keep = app.gif_keep.max(1) as usize;
    let remove = app.gif_remove.max(1) as usize;

    ui.heading("GIF Düzenle");
    ui.label(
        egui::RichText::new("Kareleri azalt, sırayı çevir, yeniden kodla.")
            .color(theme::TEXT_DIM)
            .size(12.0),
    );
    ui.add_space(8.0);

    card(ui, |ui| {
        ui.horizontal(|ui| {
            if ui.button("GIF Aç…").clicked() {
                open_clicked = true;
            }
            match (&selected, &app.gif_info) {
                (Some(p), _) => {
                    let name = std::path::Path::new(p)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| p.clone());
                    ui.label(egui::RichText::new(name).color(theme::TEXT));
                }
                (None, _) => {
                    ui.label(egui::RichText::new("Henüz dosya seçilmedi").color(theme::TEXT_DIM));
                }
            }
        });
        if let Some(p) = &selected {
            ui.label(egui::RichText::new(p).color(theme::TEXT_DIM).size(11.0));
        }
    });

    let Some(info) = app.gif_info else {
        if open_clicked {
            open_gif(app);
        }
        ui.add_space(8.0);
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
            ui.label(
                egui::RichText::new("Kare Bilgisi")
                    .color(theme::ACCENT)
                    .strong(),
            );
            ui.label(
                egui::RichText::new(format!("{} × {}  ·  {} kare", info.0, info.1, info.2))
                    .color(theme::TEXT),
            );
        });
        ui.add_space(6.0);
        let max = ui.available_width().min(560.0);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(max, max * 0.5625), egui::Sense::hover());
        ui.painter().rect_filled(rect, 6.0, theme::BG);
        if let Some(tex) = preview_texture(ui.ctx()) {
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
                egui::RichText::new("Önizleme: ilk kare").color(theme::TEXT_DIM).size(11.0),
            );
        } else if !preview_ready {
            ui.label(
                egui::RichText::new("Önizleme hazırlanıyor…").color(theme::TEXT_DIM),
            );
        }
    });

    // ── İşlemler ────────────────────────────────────────────────────────────
    ui.add_space(8.0);
    card(ui, |ui| {
        ui.label(egui::RichText::new("İşlemler").color(theme::ACCENT).strong());
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Koru:");
            ui.add(egui::DragValue::new(&mut app.gif_keep).range(1..=100).speed(1.0));
            ui.label("Atla:");
            ui.add(egui::DragValue::new(&mut app.gif_remove).range(1..=100).speed(1.0));
            if ui.button("Kare azalt").clicked() {
                apply_clicked = true;
            }
        });
        ui.add_space(4.0);
        ui.checkbox(&mut app.gif_reverse, "Ters çevir");
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button("İlk kareyi göster").clicked() {
                show_first_frame(app);
            }
            if ui.button("Kaydet…").clicked() {
                save_clicked = true;
            }
        });
    });

    if open_clicked {
        open_gif(app);
    }
    if apply_clicked {
        apply_ops(app, keep, remove);
    }
    if save_clicked {
        save_gif(app, keep, remove);
    }
}

/// Seçilen (henüz açılmamış) dosya yolu.
fn selection() -> &'static Mutex<Option<String>> {
    static S: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(None))
}

/// (hazırlanmış oturum kimliği, doku tutamacı).
fn preview_state() -> &'static Mutex<(u64, Option<egui::TextureHandle>)> {
    static S: OnceLock<Mutex<(u64, Option<egui::TextureHandle>)>> = OnceLock::new();
    S.get_or_init(|| Mutex::new((0, None)))
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

/// GIF'i diskten çözer ve `app.gif_info` ile panel oturumunu tazeler.
fn open_gif(app: &mut ScreenForgeApp) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("GIF", &["gif"])
        .pick_file()
    else {
        return;
    };
    let path = path.to_string_lossy().to_string();

    match decode(&path) {
        Ok(g) => {
            app.gif_info = Some((g.width, g.height, g.frames.len()));
            app.notify(format!("GIF açıldı: {} kare", g.frames.len()));
            app.status = format!("GIF: {path}");
            *selection().lock().unwrap() = Some(path.clone());
            *session().lock().unwrap() = Some(g);
            *preview_state().lock().unwrap() = (0, None); // önizlemeyi yeniden kur
            session_id().fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        Err(e) => {
            app.gif_info = None;
            app.notify(format!("GIF açılamadı: {e}"));
        }
    }
}

/// Kare azaltma + ters çevirmeyi yüklenen karelere kısa süreliğine uygular,
/// `app.gif_info` kare sayısını günceller, önizlemeyi tazeler.
fn apply_ops(app: &mut ScreenForgeApp, keep: usize, remove: usize) {
    let mut guard = session().lock().unwrap();
    let Some(g) = guard.as_mut() else {
        app.notify("Önce bir GIF açın");
        return;
    };
    let before = g.frames.len();
    let edit = to_editor_frames(g);
    let edit = frame_ops::reduce(&edit, keep.max(1), remove.max(1), DelayMergeMode::Distribute);
    let edit = if app.gif_reverse { frame_ops::reverse(&edit) } else { edit };
    let after = edit.len();
    drop(edit);

    let counted = (g.width, g.height, after);
    app.gif_info = Some(counted);
    *preview_state().lock().unwrap() = (0, None);
    session_id().fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    app.notify(format!("Kare azalt: {before} → {after}"));
}

/// İlk kareyi önizlemeye alır.
fn show_first_frame(app: &mut ScreenForgeApp) {
    let guard = session().lock().unwrap();
    let Some(g) = guard.as_ref() else {
        app.notify("Önce bir GIF açın");
        return;
    };
    app.gif_info = Some((g.width, g.height, g.frames.len()));
    drop(guard);
    *preview_state().lock().unwrap() = (0, None);
    session_id().fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    app.notify("İlk kare önizlemede");
}

/// İşlenmiş GIF'i seçilen yola yazar.
fn save_gif(app: &mut ScreenForgeApp, keep: usize, remove: usize) {
    let src = selection().lock().unwrap().clone();
    let Some(src) = src else {
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

    match encode(&src, &out, keep.max(1), remove.max(1), app.gif_reverse) {
        Ok(n) => {
            app.status = format!("GIF kaydedildi: {out}");
            app.notify(format!("Kaydedildi ({n} kare): {out}"));
        }
        Err(e) => app.notify(format!("Kaydedilemedi: {e}")),
    }
}

/// Önizleme dokusunu (yoksa) kurar; `None` ise henüz gösterilecek kare yok.
fn preview_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let sid = session_id().load(std::sync::atomic::Ordering::Relaxed);
    let first = {
        let guard = session().lock().unwrap();
        let g = guard.as_ref()?;
        (g.width, g.height, g.frames.first().map(|f| f.0.clone())?)
    };
    let mut state = preview_state().lock().unwrap();
    if state.0 != sid {
        state.0 = sid;
        state.1 = None;
    }
    if state.1.is_none() {
        state.1 = Some(upload_bgra(ctx, "gif-preview", first.0, first.1, first.2));
    }
    state.1.clone()
}

/// GIF'i BGRA kareler + gecikmeler olarak çözer.
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

        // Kareler tuval üzerine bindirilir; tam tuval çözümü gecikme uygular.
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
        path: path.to_string(),
        width,
        height,
        frames,
    })
}

fn to_editor_frames(g: &LoadedGif) -> Vec<std::sync::Arc<EditorFrame>> {
    g.frames
        .iter()
        .map(|(bgra, delay)| EditorFrame::new(bgra.clone(), *delay, Default::default()))
        .collect()
}

/// `gifedit::encode` ile aynı akış: işlemleri uygula, yeniden kodla.
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
    let writer = std::io::BufWriter::new(file);
    let mut gif = GifFile::new(writer);
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

/// BGRA'yı dokuya yükler.
fn upload_bgra(
    ctx: &egui::Context,
    name: &str,
    width: i32,
    height: i32,
    bgra: Vec<u8>,
) -> egui::TextureHandle {
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for px in bgra.chunks_exact(4) {
        rgba.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
    }
    let img = egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &rgba);
    ctx.load_texture(name, img, egui::TextureOptions::LINEAR)
}

// ── Ekran görüntüsü çıkarma yardımcısı (geliştirme için) ────────────────────
// Oturumdaki ilk kareyi yeni bir dokuya kopyalar; mevcut dokuya yazmak egui
// backend'lerinde garanti değil, bu yüzden yeni doku tahsis edilir.
#[allow(dead_code)]
fn preview_via_texture_manager(ctx: &egui::Context, img: egui::ColorImage) -> egui::TextureId {
    ctx.tex_manager()
        .write()
        .alloc("gif-preview".to_string(), img, egui::TextureOptions::LINEAR)
}
