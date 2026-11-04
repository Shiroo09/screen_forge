//! Serbest ("kolaj") görünümü: boş tuval, üzerine yerleştirilen görsel öğeler.
//!
//! Eski overlay'deki `CaptureMode.Free` sahnesinin sade Rust karşılığı: panodan
//! ya da dosyadan görsel ekle, sürükleyerek taşı, tekerlekle ölçekle, PNG aktar.

use crate::app::{RecentItem, ScreenForgeApp};
use crate::theme;
use egui::{Color32, RichText, Sense};

/// Tuvaldeki bir görsel. Konum ve ölçek tuval noktası (point) cinsindendir.
pub struct CollageItem {
    pub bgra: Vec<u8>,
    pub w: i32,
    pub h: i32,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    /// GPU önbelleği; ilk çizimde kurulur, `bgra` sabit olduğu için tazelenmez.
    tex: Option<egui::TextureHandle>,
}

/// Buton tıklamaları; çizim bittikten sonra uygulanır (app ödünç alınmışken).
enum Action {
    Paste,
    Add,
    Clear,
    Export,
}

pub fn ui(app: &mut ScreenForgeApp, ui: &mut egui::Ui) {
    let mut action = None;

    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        if theme::accent_button(ui, button("Panodan yapıştır", Color32::WHITE)).clicked() {
            action = Some(Action::Paste);
        }
        if theme::surface_button(ui, button("Görsel ekle", theme::TEXT)).clicked() {
            action = Some(Action::Add);
        }
        if theme::surface_button(ui, button("Temizle", theme::TEXT)).clicked() {
            action = Some(Action::Clear);
        }
        if theme::surface_button(ui, button("Dışa aktar", theme::TEXT)).clicked() {
            action = Some(Action::Export);
        }
        ui.label(
            RichText::new("Sürükle: taşı · Tekerlek: ölçekle · Ctrl+V: panodan yapıştır")
                .color(theme::TEXT_DIM)
                .size(12.0),
        );
    });
    ui.add_space(8.0);

    // Ctrl+V, düğmeyle aynı işi yapar (eski overlay kısayolu).
    if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::V)) {
        action = Some(Action::Paste);
    }

    canvas(app, ui);

    if let Some(action) = action {
        run(app, action);
    }
}

fn button(label: &str, color: Color32) -> impl FnOnce() -> egui::Button<'static> {
    let label = label.to_string();
    move || {
        egui::Button::new(RichText::new(label).size(14.0).color(color))
            .min_size(egui::vec2(140.0, 36.0))
            .corner_radius(6.0)
    }
}

/// Koyu tuval (#12151E) ve üzerindeki öğeler; taşıma/ölçekleme/seçim burada.
// Index-based loops here mirror the item order (z-order); a map/iter rewrite
// would obscure that. The `map_or` chains read more directly than `is_some_and`.
#[allow(clippy::needless_range_loop, clippy::unnecessary_map_or)]
fn canvas(app: &mut ScreenForgeApp, ui: &mut egui::Ui) {
    let size = egui::vec2(ui.available_width(), ui.available_height().max(240.0));
    let (rect, canvas_resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 8.0, theme::BG);

    let sel_id = ui.scope_id().with("collage_selected");
    let mut sel: usize = ui.data(|d| d.get_temp(sel_id)).unwrap_or(usize::MAX);

    let items = &mut app.collage_items;
    let over_canvas = ui
        .input(|i| i.pointer.hover_pos())
        .is_some_and(|p| rect.contains(p));
    let scroll = if over_canvas {
        ui.input(|i| i.smooth_scroll_delta.y)
    } else {
        0.0
    };

    let mut hover: Option<usize> = None;
    let mut clicked_item = false;
    let mut raise: Option<usize> = None;

    // 0..n sırası: en son kaydedilen (en üstteki) öğe girdiyi kazanır.
    for i in 0..items.len() {
        let (w, h, x, y, scale) = {
            let it = &items[i];
            (it.w as f32, it.h as f32, it.x, it.y, it.scale)
        };
        let irect = egui::Rect::from_min_size(rect.min + egui::vec2(x, y), egui::vec2(w, h) * scale);
        let hit = irect.intersect(rect);
        if hit.width() <= 0.0 || hit.height() <= 0.0 {
            continue; // Tamamen tuval dışında: girdiyi çalmasın.
        }
        let resp = ui.interact(hit, sel_id.with(i), Sense::click_and_drag());
        if resp.hovered() {
            hover = Some(i);
        }
        if resp.clicked() {
            sel = i;
            clicked_item = true;
        }
        if resp.drag_started() {
            sel = i;
            clicked_item = true;
            raise = Some(i);
        }
        if resp.dragged() {
            let d = resp.drag_delta();
            items[i].x += d.x;
            items[i].y += d.y;
        }
    }

    // Tekerlek: imlecin üstündeki (yoksa seçili) öğeyi ölçekle.
    if scroll != 0.0 {
        let target = hover.or(if sel != usize::MAX { Some(sel) } else { None });
        if let Some(i) = target.filter(|i| *i < items.len()) {
            let factor = (scroll * 0.0015).exp();
            items[i].scale = (items[i].scale * factor).clamp(0.05, 8.0);
        }
    }

    // Sürüklenen öğeyi öne al (z-sırası, eski overlay'deki gibi).
    if let Some(i) = raise {
        if i + 1 < items.len() {
            let it = items.remove(i);
            items.push(it);
            sel = items.len() - 1;
        }
    }

    if canvas_resp.clicked() && !clicked_item {
        sel = usize::MAX;
    }

    for i in 0..items.len() {
        let (w, h, x, y, scale) = {
            let it = &items[i];
            (it.w as f32, it.h as f32, it.x, it.y, it.scale)
        };
        let size = egui::vec2(w, h) * scale;
        let irect = egui::Rect::from_min_size(rect.min + egui::vec2(x, y), size);
        if !irect.intersects(rect) || size.x < 1.0 || size.y < 1.0 {
            continue;
        }
        let tex = texture(&mut items[i], ui.ctx());
        painter.image(
            tex.id(),
            irect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    // Seçim çerçevesi öğelerin üstünde kalsın.
    if sel < items.len() {
        let it = &items[sel];
        let irect = egui::Rect::from_min_size(
            rect.min + egui::vec2(it.x, it.y),
            egui::vec2(it.w as f32, it.h as f32) * it.scale,
        );
        painter.rect_stroke(
            irect,
            0.0,
            egui::Stroke::new(2.0, theme::ACCENT),
            egui::StrokeKind::Middle,
        );
    }

    if items.is_empty() {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Tuval boş — panodan yapıştır veya görsel ekle",
            egui::FontId::proportional(15.0),
            theme::TEXT_DIM,
        );
    }

    ui.data_mut(|d| d.insert_temp(sel_id, sel));
}

/// Öğenin BGRA tamponundan egui dokusu üretir (yalnız ilk çağrıda).
fn texture(item: &mut CollageItem, ctx: &egui::Context) -> egui::TextureHandle {
    use std::sync::atomic::{AtomicU64, Ordering};
    static TEX_SEQ: AtomicU64 = AtomicU64::new(0);

    if item.tex.is_none() {
        let mut rgba = Vec::with_capacity((item.w * item.h * 4) as usize);
        for px in item.bgra.chunks_exact(4) {
            rgba.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
        }
        let img =
            egui::ColorImage::from_rgba_unmultiplied([item.w as usize, item.h as usize], &rgba);
        // Ad tekil olmalı: egui aynı adlı eski dokuyu serbest bırakır.
        let name = format!("collage_tex_{}", TEX_SEQ.fetch_add(1, Ordering::Relaxed));
        item.tex = Some(ctx.load_texture(name, img, egui::TextureOptions::LINEAR));
    }
    item.tex.clone().unwrap()
}

fn run(app: &mut ScreenForgeApp, action: Action) {
    match action {
        Action::Paste => match clipboard_image() {
            Ok((w, h, bgra)) => {
                push_item(app, w, h, bgra);
                app.notify("panodaki görsel yerleştirildi");
            }
            Err(e) => app.notify(format!("pano: {e}")),
        },
        Action::Add => {
            let Some(path) = rfd::FileDialog::new()
                .add_filter("Görsel", &["png", "jpg", "jpeg", "webp", "bmp"])
                .pick_file()
            else {
                return;
            };
            match load_image(&path) {
                Ok((w, h, bgra)) => {
                    push_item(app, w, h, bgra);
                    app.notify("görsel eklendi");
                }
                Err(e) => app.notify(format!("hata: {e}")),
            }
        }
        Action::Clear => {
            app.collage_items.clear();
            app.collage_items.shrink_to_fit();
            app.notify("tuval temizlendi");
        }
        Action::Export => export(app),
    }
}

/// Öğeyi hafif kademeli bir konuma, tuvale sığacak ölçekle ekler.
fn push_item(app: &mut ScreenForgeApp, w: i32, h: i32, bgra: Vec<u8>) {
    let n = app.collage_items.len() as f32;
    // Büyük yakalamalar tuvali doldurmasın: uzun kenarı ~640 noktaya indir.
    let scale = (640.0 / w.max(1) as f32).min(1.0);
    app.collage_items.push(CollageItem {
        bgra,
        w,
        h,
        x: 24.0 + n * 18.0,
        y: 24.0 + n * 18.0,
        scale,
        tex: None,
    });
}

fn load_image(path: &std::path::Path) -> Result<(i32, i32, Vec<u8>), String> {
    let img = image::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut bgra = rgba.into_raw();
    for px in bgra.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    Ok((w as i32, h as i32, bgra))
}

fn export(app: &mut ScreenForgeApp) {
    if app.collage_items.is_empty() {
        app.notify("tuval boş");
        return;
    }
    let (w, h, rgba) = match composite(&app.collage_items) {
        Ok(v) => v,
        Err(e) => {
            app.notify(format!("hata: {e}"));
            return;
        }
    };
    let Some(path) = rfd::FileDialog::new()
        .set_file_name("kolaj.png")
        .add_filter("PNG", &["png"])
        .save_file()
    else {
        return;
    };
    let Some(img) = image::RgbaImage::from_raw(w as u32, h as u32, rgba) else {
        app.notify("hata: görüntü boyutu uyuşmuyor");
        return;
    };
    match img.save(&path) {
        Ok(()) => {
            let shown = path.display().to_string();
            app.recents.insert(
                0,
                RecentItem {
                    label: format!("Kolaj · {w}×{h}"),
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

/// Öğeleri koyu tuvale bindirip tek RGBA tamponu üretir (en-boy yakın komşu).
fn composite(items: &[CollageItem]) -> Result<(i32, i32, Vec<u8>), String> {
    let (mut min_x, mut min_y) = (0.0f32, 0.0f32);
    let (mut max_x, mut max_y) = (1.0f32, 1.0f32);
    for it in items {
        min_x = min_x.min(it.x);
        min_y = min_y.min(it.y);
        max_x = max_x.max(it.x + it.w as f32 * it.scale);
        max_y = max_y.max(it.y + it.h as f32 * it.scale);
    }
    let w = (max_x - min_x).ceil().max(1.0) as i32;
    let h = (max_y - min_y).ceil().max(1.0) as i32;
    if w > 16384 || h > 16384 {
        return Err("tuval çok büyük (16384 px sınırı)".to_string());
    }

    // Arka plan #12151E, opak (BGRA).
    let mut out = vec![0u8; (w * h * 4) as usize];
    for px in out.chunks_exact_mut(4) {
        px[0] = 0x1E;
        px[1] = 0x15;
        px[2] = 0x12;
        px[3] = 0xFF;
    }

    for it in items {
        let ox = (it.x - min_x).round() as i32;
        let oy = (it.y - min_y).round() as i32;
        let dw = (it.w as f32 * it.scale).round().max(1.0) as i32;
        let dh = (it.h as f32 * it.scale).round().max(1.0) as i32;
        for dy in 0..dh {
            let ty = oy + dy;
            if ty < 0 || ty >= h {
                continue;
            }
            let sy = ((dy as f32 + 0.5) / it.scale) as i32;
            let sy = sy.clamp(0, it.h - 1) as usize;
            for dx in 0..dw {
                let tx = ox + dx;
                if tx < 0 || tx >= w {
                    continue;
                }
                let sx = ((dx as f32 + 0.5) / it.scale) as i32;
                let sx = sx.clamp(0, it.w - 1) as usize;
                let s = (sy * it.w as usize + sx) * 4;
                let d = ((ty as usize * w as usize) + tx as usize) * 4;
                let Some(src) = it.bgra.get(s..s + 4) else {
                    continue;
                };
                let a = src[3] as u32;
                if a == 0 {
                    continue;
                }
                if a == 255 {
                    out[d] = src[0];
                    out[d + 1] = src[1];
                    out[d + 2] = src[2];
                } else {
                    for k in 0..3 {
                        let bg = out[d + k] as u32;
                        out[d + k] = ((src[k] as u32 * a + bg * (255 - a)) / 255) as u8;
                    }
                }
                out[d + 3] = 0xFF;
            }
        }
    }

    // BGRA → RGBA
    for px in out.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    Ok((w, h, out))
}

// --- Pano okuma (Win32 CF_DIB) ---------------------------------------------

#[cfg(windows)]
fn clipboard_image() -> Result<(i32, i32, Vec<u8>), String> {
    imp::read_dib()
}

#[cfg(not(windows))]
fn clipboard_image() -> Result<(i32, i32, Vec<u8>), String> {
    Err("pano okuma yalnızca Windows'ta desteklenir".to_string())
}

/// `sf_capture` panoya CF_DIB yazıyor; okuma da aynı biçimden yapılır.
#[cfg(windows)]
mod imp {
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::Graphics::Gdi::BITMAPINFOHEADER;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, OpenClipboard,
    };
    use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
    use windows::Win32::System::Ole::CF_DIB;

    pub fn read_dib() -> Result<(i32, i32, Vec<u8>), String> {
        unsafe {
            OpenClipboard(None).map_err(|e| format!("OpenClipboard: {e}"))?;
            struct Guard;
            impl Drop for Guard {
                fn drop(&mut self) {
                    unsafe {
                        let _ = CloseClipboard();
                    }
                }
            }
            let _guard = Guard;

            let handle = GetClipboardData(CF_DIB.0 as u32)
                .map_err(|_| "panoda görsel yok".to_string())?;
            let hglobal = HGLOBAL(handle.0);
            let ptr = GlobalLock(hglobal);
            if ptr.is_null() {
                return Err("GlobalLock başarısız".to_string());
            }

            let result = decode(ptr as *const u8);
            let _ = GlobalUnlock(hglobal);
            result
        }
    }

    unsafe fn decode(ptr: *const u8) -> Result<(i32, i32, Vec<u8>), String> {
        let header = &*(ptr as *const BITMAPINFOHEADER);
        if header.biCompression != 0 {
            return Err("sıkıştırılmış DIB desteklenmiyor".to_string());
        }
        let bpp = header.biBitCount as usize;
        if bpp != 24 && bpp != 32 {
            return Err(format!("desteklenmeyen bit derinliği: {bpp}"));
        }
        let w = header.biWidth;
        let top_down = header.biHeight < 0;
        let h = header.biHeight.abs();
        if w <= 0 || h <= 0 {
            return Err("geçersiz DIB boyutu".to_string());
        }

        // `sf_capture` kalıcı panoyu bırakır; CF_DIB verisi kalıcı olduğu için
        // burada bir kopya çıkarmak yeterli, sahipliği clipboard'da kalır.
        let src = ptr.add(std::mem::size_of::<BITMAPINFOHEADER>());
        let stride = (w as usize * bpp).div_ceil(32) * 4;
        let mut bgra = Vec::with_capacity((w * h * 4) as usize);
        for row in 0..h as usize {
            let y = if top_down { row } else { h as usize - 1 - row };
            let row_ptr = src.add(y * stride);
            for x in 0..w as usize {
                let p = row_ptr.add(x * bpp / 8);
                let b = *p;
                let g = *p.add(1);
                let r = *p.add(2);
                // Ekran yakalamalarında alfa 0 gelir; saydam sayma, opak kabul et.
                let a = if bpp == 32 {
                    let v = *p.add(3);
                    if v == 0 {
                        0xFF
                    } else {
                        v
                    }
                } else {
                    0xFF
                };
                bgra.extend_from_slice(&[b, g, r, a]);
            }
        }
        Ok((w, h, bgra))
    }
}
