//! GIF editörü: GIF aç, kareleri küçük resim şeridinden seç, oynat/duraklat,
//! kare işlemlerini uygula (azalt / gecikme / ters çevir), yeniden kodla.
//!
//! Çözme ve kodlama `sf-cli/src/gifedit.rs`'teki çalışan akışın aynısı; kare
//! işlemleri ortak `sf_edit::frame_ops` üzerinden yapılır. Yüklenen kareler
//! `app.rs`'e ağır alan eklemeden modül düzeyinde tutulur.
//!
//! Önizleme dokusu, küçük resim önbelleği ve oynatma durumu da buradadır.
//! Önbellek anahtarı karenin **sıkıştırılmış tamponunun adresi**dir: `with_delay`
//! aynı tamponu paylaştığı için gecikme düzenlemek küçük resimleri geçersiz
//! kılmaz, ama kare sırası/kümesi değişince (reduce, reverse) adres de değişir
//! ve yalnız etkilenen girişler yeniden çözülür. Oturum içinde yeni tampon
//! üretilmez; yalnız `open_gif` üretir, o da oturum kimliğini artırıp önbelleği
//! tümden temizler.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use sf_edit::frame_ops::{self, DelayMergeMode, DuplicateRemoval};
use sf_edit::{image_ops, EditorFrame, FrameInput};
use sf_gif::gif_file::{GifFile, Rect};

use crate::app::ScreenForgeApp;
use crate::theme;

/// Küçük resim hedef genişliği (px). C# `ThumbnailCache` varsayılanı 76.
const THUMB_WIDTH: i32 = 76;
/// Küçük resmin en fazla yüksekliği — çok uzun GIF'lerde şerit taşmasın.
const THUMB_MAX_HEIGHT: f32 = 90.0;
/// Zaman çizelgesi şeridinin en fazla yüksekliği.
const STRIP_HEIGHT: f32 = 110.0;
/// Önbellekte tutulan en fazla küçük resim.
const THUMB_CAP: usize = 512;
/// Gecikmesi 0 olan kareler için en kısa oynatma adımı (sn).
const MIN_STEP: f32 = 0.01;

/// Çözülmüş GIF: tuval boyutu + kareler. Kare pikselleri `EditorFrame` içinde
/// sıkıştırılmış durur (bkz. `sf_gif::frame_store`); ikinci bir çözücü yoktur.
struct LoadedGif {
    width: i32,
    height: i32,
    frames: Vec<Arc<EditorFrame>>,
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

/// Oturum kimliği — yalnız yeni bir GIF açıldığında artar.
fn session_id() -> &'static AtomicU64 {
    static S: AtomicU64 = AtomicU64::new(0);
    &S
}

/// Zaman çizelgesinde seçili kare.
fn selected() -> &'static AtomicUsize {
    static S: AtomicUsize = AtomicUsize::new(0);
    &S
}

/// Oynatma: duraklatma anında kalan süre + kullanıcı tercihleri.
struct Playback {
    playing: bool,
    elapsed: f32,
    looping: bool,
    fps: i32,
}

impl Default for Playback {
    fn default() -> Self {
        Self {
            playing: false,
            elapsed: 0.0,
            looping: true,
            fps: 12,
        }
    }
}

fn playback() -> &'static Mutex<Playback> {
    static S: OnceLock<Mutex<Playback>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Playback::default()))
}

/// Büyük önizleme dokusu ve hangi kareye ait olduğu (oturum, dizin, paket adresi).
type PreviewKey = (u64, usize, u64);

fn preview_state() -> &'static Mutex<(PreviewKey, Option<egui::TextureHandle>)> {
    static S: OnceLock<Mutex<(PreviewKey, Option<egui::TextureHandle>)>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(((0, 0, 0), None)))
}

/// Kare küçük resimleri: kare dizini → (paket tamponunun adresi, doku).
#[derive(Default)]
struct ThumbCache {
    session: u64,
    size: (i32, i32),
    entries: HashMap<usize, (u64, egui::TextureHandle)>,
}

impl ThumbCache {
    fn get(
        &mut self,
        ctx: &egui::Context,
        sid: u64,
        index: usize,
        size: (i32, i32),
    ) -> Option<egui::TextureHandle> {
        // Yeni oturum ya da tuval boyutu değişti: eski küçük resimler yanlış
        // en-boy oranını gösterir, tümünü at.
        if self.session != sid || self.size != size {
            self.session = sid;
            self.size = size;
            self.entries.clear();
        }

        let frame = session().lock().unwrap().as_ref()?.frames.get(index)?.clone();
        let key = frame.packed().as_ptr() as u64;
        if let Some((cached_key, tex)) = self.entries.get(&index) {
            if *cached_key == key {
                return Some(tex.clone());
            }
        }

        let (w, h) = size;
        let (tw, th) = thumb_size(w, h);
        let small = image_ops::resize_pixels(&frame.pixels_uncached(), w, h, tw, th);
        let img = egui::ColorImage::from_rgba_unmultiplied(
            [tw as usize, th as usize],
            &bgra_to_rgba(&small),
        );
        // Ad, anahtarı taşır: eski doku yeni veriyle ezilmesin.
        let name = format!("gif-thumb-{index}-{key}");
        let tex = ctx.load_texture(name, img, egui::TextureOptions::LINEAR);

        if self.entries.len() >= THUMB_CAP {
            if let Some(victim) = eviction_victim(self.entries.keys(), index) {
                self.entries.remove(&victim);
            }
        }
        self.entries.insert(index, (key, tex.clone()));
        Some(tex)
    }
}

fn thumb_cache() -> &'static Mutex<ThumbCache> {
    static S: OnceLock<Mutex<ThumbCache>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(ThumbCache::default()))
}

/// Küçük resmin hedef piksel boyutu: en-boy korunur, en fazla 76×90.
fn thumb_size(w: i32, h: i32) -> (i32, i32) {
    let scale = (THUMB_WIDTH as f32 / w as f32)
        .min(THUMB_MAX_HEIGHT / h as f32)
        .min(1.0);
    (
        ((w as f32 * scale).round() as i32).max(1),
        ((h as f32 * scale).round() as i32).max(1),
    )
}

/// Önbellek dolduğunda atılacak kare: istenen kareden en uzaktaki. Zaman
/// çizelgesinde gezinme yerel olduğu için bu, LRU'ya yakın davranır.
fn eviction_victim<'a>(keys: impl Iterator<Item = &'a usize>, index: usize) -> Option<usize> {
    keys.max_by_key(|&&i| i.abs_diff(index)).copied()
}

/// Görünür pencerenin kapsadığı kare aralığı (bir hücre pay bırakılır).
/// Yalnız bu aralıktaki kareler çözülür; şerit sanallaştırmasının çekirdeği.
fn visible_range(view_min_x: f32, view_max_x: f32, cell: f32, count: usize) -> (usize, usize) {
    if cell <= 0.0 || count == 0 {
        return (0, 0);
    }
    let first = (view_min_x / cell).floor().max(0.0) as usize;
    let last = (((view_max_x / cell).ceil() as usize).saturating_add(1)).min(count);
    (first.min(count), last.max(first.min(count)))
}

/// Dizini geçerli aralığa sabitler (önbellek anahtarından ayrı, saf).
fn pin(index: usize, count: usize) -> usize {
    if count == 0 {
        0
    } else {
        index.min(count - 1)
    }
}

fn current_index(count: usize) -> usize {
    pin(selected().load(Ordering::Relaxed), count)
}

fn clamp_selection(count: usize) {
    selected().store(pin(selected().load(Ordering::Relaxed), count), Ordering::Relaxed);
}

/// Oynatmanın sıradaki adımı: `(yeni dizin, oynatma sürüyor mu)`.
/// Döngü kapalıysa son karede durur; kare sayısı 2'den azsa hiç ilerlemez.
fn next_frame(current: usize, count: usize, looping: bool) -> (usize, bool) {
    if count < 2 {
        return (current.min(count.saturating_sub(1)), false);
    }
    if current + 1 < count {
        (current + 1, true)
    } else if looping {
        (0, true)
    } else {
        (current, false)
    }
}

/// Bir karenin gecikmesinden oynatma adımı (sn).
fn step_secs(delay_ms: i32) -> f32 {
    (delay_ms.max(10) as f32 / 1000.0).max(MIN_STEP)
}

/// BGRA → RGBA (egui `ColorImage` bekler).
fn bgra_to_rgba(bgra: &[u8]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(bgra.len());
    for px in bgra.chunks_exact(4) {
        rgba.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
    }
    rgba
}

/// Panel girişi. İçerik min pencere yüksekliğini (480 px) aştığı için dikey
/// kaydırma sarmalayıcısı var; `settings` paneliyle aynı desen.
pub fn ui(app: &mut ScreenForgeApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| body(app, ui));
}

fn body(app: &mut ScreenForgeApp, ui: &mut egui::Ui) {
    ui.heading("GIF Düzenle");
    ui.label(
        egui::RichText::new("Kareleri azalt, gecikmeyi ayarla, oynat, yeniden kodla.")
            .color(theme::TEXT_DIM)
            .size(12.0),
    );
    ui.add_space(8.0);

    let mut pick = false;
    card(ui, |ui| {
        ui.horizontal(|ui| {
            if ui.button("GIF Aç...").clicked() {
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

    if pick {
        open_gif(app);
    }

    let Some((w, h, count)) = app.gif_info else {
        ui.label(
            egui::RichText::new("Önizleme için bir GIF açın.")
                .color(theme::TEXT_DIM)
                .size(12.0),
        );
        return;
    };

    let sid = session_id().load(Ordering::Relaxed);
    let delays: Vec<i32> = match session().lock().unwrap().as_ref() {
        Some(g) => g.frames.iter().map(|f| f.delay).collect(),
        None => Vec::new(),
    };

    // ── Oynatma ilerletmesi ─────────────────────────────────────────────────
    // Gerçek süre biriktirilir; her karenin kendi gecikmesi kadar beklenir.
    let mut follow = false;
    {
        let mut pb = playback().lock().unwrap();
        if pb.playing && count > 1 {
            // Duraklatılmış/donmuş pencerede birikimi sınırla: 0.25 sn'lik bir
            // sıçrama en fazla birkaç kare atlar.
            pb.elapsed += ui.ctx().input(|i| i.stable_dt).min(0.25);
            loop {
                let now = current_index(count);
                let step = step_secs(delays.get(now).copied().unwrap_or(10));
                if pb.elapsed < step {
                    break;
                }
                pb.elapsed -= step;
                let (next, cont) = next_frame(now, count, pb.looping);
                if next != now {
                    selected().store(next, Ordering::Relaxed);
                    follow = true;
                }
                if !cont {
                    pb.playing = false;
                    pb.elapsed = 0.0;
                    break;
                }
            }
            if pb.playing {
                let now = current_index(count);
                let step = step_secs(delays.get(now).copied().unwrap_or(10));
                ui.ctx().request_repaint_after(Duration::from_secs_f32(step));
            }
        }
    }

    let cur = current_index(count);

    // ── Önizleme + oynatma çubuğu ───────────────────────────────────────────
    let mut toggle_play = false;
    let mut delay_edit: i32 = 0;
    let mut set_delay: Option<i32> = None;
    let mut set_fps: Option<i32> = None;
    card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Kare Bilgisi").color(theme::ACCENT).strong());
            ui.label(
                egui::RichText::new(format!("{w} × {h}  ·  {count} kare")).color(theme::TEXT),
            );
        });
        ui.add_space(6.0);

        {
            let mut pb = playback().lock().unwrap();
            ui.horizontal(|ui| {
                let label = if pb.playing { "Duraklat" } else { "Oynat" };
                if ui.add_enabled(count > 1, egui::Button::new(label)).clicked() {
                    toggle_play = true;
                }
                ui.checkbox(&mut pb.looping, "Döngü");
                ui.add_space(6.0);
                ui.label("FPS:");
                let resp = ui.add(egui::DragValue::new(&mut pb.fps).range(1..=60).speed(1.0));
                if resp.changed() {
                    set_fps = Some(pb.fps);
                }
            });
        }
        ui.add_space(4.0);

        let max = ui.available_width().min(560.0);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(max, max * 0.5625), egui::Sense::hover());
        ui.painter().rect_filled(rect, 6.0, theme::BG);
        match current_frame(cur) {
            Some(frame) => {
                let key = frame.packed().as_ptr() as u64;
                let tex = preview_texture(ui.ctx(), sid, cur, key, &frame.pixels_uncached(), w, h);
                let size = tex.size_vec2();
                let s = (rect.width() / size.x).min(rect.height() / size.y);
                let shown = egui::Rect::from_center_size(rect.center(), size * s);
                ui.painter().image(
                    tex.id(),
                    shown,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
            None => {
                ui.label(egui::RichText::new("Önizleme hazırlanıyor...").color(theme::TEXT_DIM));
            }
        }

        let cur_delay = delays.get(cur).copied().unwrap_or(0);
        delay_edit = cur_delay;
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "Kare {}/{}  ·  gecikme: {} ms",
                    cur + 1,
                    count,
                    cur_delay
                ))
                .color(theme::TEXT_DIM)
                .size(11.0),
            );
            ui.add_space(8.0);
            ui.label("Bu kare:");
            let resp = ui.add(
                egui::DragValue::new(&mut delay_edit)
                    .range(10..=60_000)
                    .speed(5.0)
                    .suffix(" ms"),
            );
            if resp.changed() {
                set_delay = Some(delay_edit);
            }
        });
    });

    if toggle_play {
        let mut pb = playback().lock().unwrap();
        if pb.playing {
            pb.playing = false;
            pb.elapsed = 0.0;
        } else if count > 1 {
            pb.playing = true;
            pb.elapsed = 0.0;
        }
    }
    if let Some(ms) = set_delay {
        edit_frames(app, "Gecikme güncellendi", |f| {
            frame_ops::set_delay(f, &[cur], ms)
        });
    }
    if let Some(fps) = set_fps {
        edit_frames(app, format!("Tüm kareler {fps} FPS"), |f| {
            frame_ops::set_fps(f, fps)
        });
    }

    // ── Kare şeridi (zaman çizelgesi) ───────────────────────────────────────
    // Yalnız görünür hücrelerin pikselleri çözülür: uzun bir kayıtta her
    // boyamada tüm kareleri çözmek paneli kilitler.
    // shortcut: şerit yine de tüm kareler için hücre ayırır; birkaç bin karede
    // egui sanal listeye geçilmeli (şerit içine yuvalanmış dikey sanal ScrollArea).
    let mut clicked: Option<usize> = None;
    {
        let mut cache = thumb_cache().lock().unwrap();
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Kareler").color(theme::ACCENT).strong());
                ui.label(
                    egui::RichText::new("tıkla: kareyi seç").color(theme::TEXT_DIM).size(11.0),
                );
            });
            ui.add_space(6.0);
            let (tw, th) = thumb_size(w, h);
            let cell = egui::vec2(tw as f32 + 8.0, th as f32 + 26.0);
            egui::ScrollArea::horizontal()
                .id_salt("gif_timeline")
                .max_height(STRIP_HEIGHT)
                .show_viewport(ui, |ui, viewport| {
                    // Şerit tek satır; yalnız yatay eksen sanallaşır.
                    let (first, last) = visible_range(viewport.min.x, viewport.max.x, cell.x, count);
                    let (_, strip) =
                        ui.allocate_space(egui::vec2(cell.x * count as f32, cell.y));
                    for i in first..last {
                        let rect = egui::Rect::from_min_size(
                            strip.min + egui::vec2(i as f32 * cell.x, 0.0),
                            cell,
                        );
                        let Some(tex) = cache.get(ui.ctx(), sid, i, (w, h)) else {
                            continue;
                        };
                        let delay = delays.get(i).copied().unwrap_or(0);
                        let resp = thumb_cell_rect(ui, rect, i, &tex, i == cur, delay);
                        if resp.clicked() {
                            clicked = Some(i);
                        }
                        if follow && i == cur {
                            resp.scroll_to_me(Some(egui::Align::Center));
                            follow = false;
                        }
                    }
                });
        });
    }
    if let Some(i) = clicked {
        selected().store(i, Ordering::Relaxed);
        playback().lock().unwrap().elapsed = 0.0;
    }

    // ── İşlemler ────────────────────────────────────────────────────────────
    let mut apply = false;
    let mut dedup = false;
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
            ui.label("Benzerlik eşiği:");
            ui.add(egui::DragValue::new(&mut app.gif_similarity).range(0.0..=100.0).speed(0.5));
            dedup = ui.button("Yinelenenleri kaldır").clicked();
        });
        ui.add_space(6.0);
        save = ui.button("Kaydet...").clicked();
    });

    if apply {
        apply_reduce(app);
    }
    if dedup {
        apply_dedup(app);
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

/// Zaman çizelgesinde tek bir küçük resim hücresini verilen dikdörtgene çizer.
fn thumb_cell_rect(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    index: usize,
    tex: &egui::TextureHandle,
    is_selected: bool,
    delay_ms: i32,
) -> egui::Response {
    let size = tex.size_vec2();
    let resp = ui.interact(
        rect,
        egui::Id::unique(("gif-thumb", index)),
        egui::Sense::click(),
    );
    let img_rect =
        egui::Rect::from_min_size(rect.min + egui::vec2(4.0, 2.0), size);

    let painter = ui.painter();
    painter.rect_filled(img_rect, 3.0, theme::BG);
    painter.image(
        tex.id(),
        img_rect,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        egui::Color32::WHITE,
    );
    let stroke = if is_selected {
        egui::Stroke::new(2.0, theme::ACCENT)
    } else if resp.hovered() {
        egui::Stroke::new(1.0, theme::ACCENT_HOVER)
    } else {
        egui::Stroke::new(1.0, theme::BORDER)
    };
    painter.rect_stroke(img_rect, 3.0, stroke, egui::StrokeKind::Outside);
    painter.text(
        egui::pos2(rect.center().x, rect.bottom() - 11.0),
        egui::Align2::CENTER_CENTER,
        format!("{} · {} ms", index + 1, delay_ms),
        egui::FontId::proportional(9.5),
        if is_selected { theme::TEXT } else { theme::TEXT_DIM },
    );
    resp
}

/// Seçili karenin `Arc`'ı (pikselleri çözerken oturum kilidi tutulmaz).
fn current_frame(index: usize) -> Option<Arc<EditorFrame>> {
    session().lock().unwrap().as_ref()?.frames.get(index).cloned()
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
            // Yeni oturum: önbellekler ve önizleme tümden geçersiz.
            *preview_state().lock().unwrap() = ((0, 0, 0), None);
            session_id().fetch_add(1, Ordering::Relaxed);
            selected().store(0, Ordering::Relaxed);
            *playback().lock().unwrap() = Playback::default();
            if let Some(g) = session().lock().unwrap().as_ref() {
                app.gif_info = Some((g.width, g.height, count));
            }
            app.status = format!("GIF: {path}");
            app.notify(format!("GIF açıldı: {count} kare"));
        }
        Err(e) => {
            app.gif_info = None;
            app.notify(format!("GIF açılamadı: {e}"));
        }
    }
}

/// Oturumun kare listesini değiştirir ve panel sayacını tazeler.
///
/// Önbellekler kare kimliğine (paket adresi) bağlı olduğu için burada toplu
/// geçersiz kılma gerekmez: sırası değişen ya da değişen kareler anahtarını
/// değiştirir, aynı kalanlar önbellekten gelir.
fn replace_frames(app: &mut ScreenForgeApp, frames: Vec<Arc<EditorFrame>>, msg: impl Into<String>) {
    let (w, h, n) = {
        let mut guard = session().lock().unwrap();
        let Some(g) = guard.as_mut() else {
            app.notify("Önce bir GIF açın");
            return;
        };
        g.frames = frames;
        (g.width, g.height, g.frames.len())
    };
    app.gif_info = Some((w, h, n));
    clamp_selection(n);
    app.notify(msg);
}

/// Kare listesi üzerinde bir `frame_ops` işlemi uygular.
fn edit_frames(
    app: &mut ScreenForgeApp,
    msg: impl Into<String>,
    op: impl FnOnce(&[Arc<EditorFrame>]) -> Vec<Arc<EditorFrame>>,
) {
    let frames = {
        let guard = session().lock().unwrap();
        let Some(g) = guard.as_ref() else {
            app.notify("Önce bir GIF açın");
            return;
        };
        g.frames.clone()
    };
    let out = op(&frames);
    replace_frames(app, out, msg);
}

/// "Kare azalt": keep/remove kuralının sonucunu önizlemeye uygular.
/// Kaydetme aynı işlemi kaynak dosyadan yeniden uygular.
fn apply_reduce(app: &mut ScreenForgeApp) {
    let keep = app.gif_keep.max(1) as usize;
    let remove = app.gif_remove.max(1) as usize;
    let before = session().lock().unwrap().as_ref().map(|g| g.frames.len()).unwrap_or(0);
    edit_frames(app, "", |f| {
        frame_ops::reduce(f, keep, remove, DelayMergeMode::Distribute)
    });
    let after = app.gif_info.map(|(_, _, n)| n).unwrap_or(0);
    app.notify(format!("Kare azalt: {before} → {after}"));
}

/// "Yinelenenleri kaldır": benzerlik eşiğinin altındaki ardışık kareleri atar.
fn apply_dedup(app: &mut ScreenForgeApp) {
    let similarity = app.gif_similarity as f64;
    let before = session().lock().unwrap().as_ref().map(|g| g.frames.len()).unwrap_or(0);
    edit_frames(app, "", |f| {
        frame_ops::remove_duplicates(
            f,
            similarity,
            DuplicateRemoval::Last,
            DelayMergeMode::AddToPrevious,
            false,
        )
    });
    let after = app.gif_info.map(|(_, _, n)| n).unwrap_or(0);
    app.notify(format!("Yinelenenler: {before} → {after}"));
}

/// İşlenmiş GIF'i seçilen yola yazar.
///
/// Kaydetme, önizlemede duran oturumu yazar: gecikme ve yinelenen-kaldırma
/// düzenlemeleri yalnız oturumda tutulduğu için kaynak dosyayı yeniden çözmek
/// onları sessizce düşürürdü. `keep`/`remove`/`ters` bir kez daha uygulanır
/// (oturum bu adımları içermez), gecikme ve yinelenen düzeltmeleri zaten içerir.
fn save_gif(app: &mut ScreenForgeApp) {
    let Some(src) = selection().lock().unwrap().clone() else {
        app.notify("Önce bir GIF açın");
        return;
    };
    let (w, h) = match session().lock().unwrap().as_ref() {
        Some(g) => (g.width, g.height),
        None => {
            app.notify("Önce bir GIF açın");
            return;
        }
    };
    let mut dialog = rfd::FileDialog::new().add_filter("GIF", &["gif"]);
    if let Some(name) = std::path::Path::new(&src).file_name() {
        dialog = dialog.set_file_name(name.to_string_lossy());
    }
    let Some(out) = dialog.save_file() else {
        return;
    };
    let out = out.to_string_lossy().to_string();

    let keep = app.gif_keep.max(1) as usize;
    let remove = app.gif_remove.max(1) as usize;
    let reverse = app.gif_reverse;
    // Pikselleri kilit altında çöz: encode hata verirse liste yarım kalmasın.
    let result = {
        let mut guard = session().lock().unwrap();
        let Some(g) = guard.as_mut() else {
            app.notify("Önce bir GIF açın");
            return;
        };
        let mut edit = g.frames.clone();
        edit = frame_ops::reduce(&edit, keep, remove, DelayMergeMode::Distribute);
        if reverse {
            // Ters çevirme artık önizlemede de geçerli; onay kutusu sıfırlanır,
            // aksi hâlde bir sonraki kayıt tersini bir kez daha çevirir.
            g.frames = frame_ops::reverse(&edit);
            app.gif_reverse = false;
        } else {
            g.frames = edit;
        }
        encode_frames(&g.frames, w, h, &out, &app.settings.gif)
    };
    match result {
        Ok(n) => {
            app.gif_info = session()
                .lock()
                .unwrap()
                .as_ref()
                .map(|g| (g.width, g.height, g.frames.len()));
            app.status = format!("GIF kaydedildi: {out}");
            app.notify(format!("Kaydedildi ({n} kare): {out}"));
        }
        Err(e) => app.notify(format!("Kaydedilemedi: {e}")),
    }
}

/// Büyük önizleme dokusunu (gerekirse yeniden) kurar.
fn preview_texture(
    ctx: &egui::Context,
    session: u64,
    index: usize,
    key: u64,
    bgra: &[u8],
    w: i32,
    h: i32,
) -> egui::TextureHandle {
    let want: PreviewKey = (session, index, key);
    let mut state = preview_state().lock().unwrap();
    if state.0 != want {
        let img =
            egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &bgra_to_rgba(bgra));
        state.1 = Some(ctx.load_texture(
            format!("gif-preview-{index}-{key}"),
            img,
            egui::TextureOptions::LINEAR,
        ));
        state.0 = want;
    }
    state.1.clone().expect("az önce kuruldu")
}

/// GIF'i tuval üzerine bindirilmiş kareler + gecikmeler olarak çözer.
/// Kareler `sf-gif`'in sıkıştırıcısıyla paketlenir; ayrı bir çözücü yoktur.
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
        frames.push(EditorFrame::new(
            canvas.clone(),
            frame.delay as i32 * 10, // 1/100 s → ms
            FrameInput::default(),
        ));
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

/// Verilen kareleri tuval boyutunda yeniden kodlar.
fn encode_frames(
    edit: &[Arc<EditorFrame>],
    width: i32,
    height: i32,
    out: &str,
    s: &sf_settings::GifSettings,
) -> Result<usize, String> {
    let file = std::fs::File::create(out).map_err(|e| format!("oluştur {out}: {e}"))?;
    let mut gif = GifFile::new(std::io::BufWriter::new(file));
    gif.set_canvas_size(width, height);
    gif.maximum_number_color = s.color_count.clamp(2, 256);
    gif.quantizer_type = if s.quantizer.eq_ignore_ascii_case("Octree") {
        sf_gif::QuantizerType::Octree
    } else {
        sf_gif::QuantizerType::Neural
    };
    gif.sampling_factor = s.sampling_factor.clamp(1, 20);
    gif.use_dithering = s.dithering;
    gif.use_global_palette = s.use_global_palette;
    if gif.use_global_palette {
        gif.build_global_palette(&edit.iter().map(|f| f.pixels_uncached()).collect::<Vec<_>>());
    }

    for frame in edit {
        gif.add_frame(
            &frame.pixels_uncached(),
            Rect {
                x: 0,
                y: 0,
                width,
                height,
            },
            frame.delay,
            false,
        );
    }
    gif.finish();
    Ok(edit.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 4×4 düz renkli bir kare (BGRA, 64 bayt).
    fn frame(value: u8, delay: i32) -> Arc<EditorFrame> {
        EditorFrame::new(vec![value; 4 * 4 * 4], delay, FrameInput::default())
    }

    fn load_session(frames: Vec<Arc<EditorFrame>>) {
        *session().lock().unwrap() = Some(LoadedGif {
            width: 4,
            height: 4,
            frames,
        });
    }

    #[test]
    fn oynatma_son_karede_durur_ya_da_basa_doner() {
        assert_eq!(next_frame(0, 4, true), (1, true));
        assert_eq!(next_frame(2, 4, false), (3, true));
        // Son kare + döngü açık: başa dön.
        assert_eq!(next_frame(3, 4, true), (0, true));
        // Son kare + döngü kapalı: dur.
        assert_eq!(next_frame(3, 4, false), (3, false));
    }

    #[test]
    fn tek_kareli_gif_oynatmaz() {
        assert_eq!(next_frame(0, 1, true), (0, false));
        assert_eq!(next_frame(0, 0, true), (0, false));
        // Bozuk indeks panik üretmemeli.
        assert_eq!(next_frame(9, 3, true), (0, true));
    }

    #[test]
    fn gecikme_ve_indeks_sinirlari() {
        assert_eq!(pin(7, 3), 2);
        assert_eq!(pin(1, 3), 1);
        assert_eq!(pin(0, 0), 0);
        // 0 gecikmeli kareler 10 ms'lik adıma düşer.
        assert_eq!(step_secs(0), 0.01);
        assert_eq!(step_secs(40), 0.04);
        assert_eq!(step_secs(-5), 0.01);
    }

    #[test]
    fn serit_yalniz_gorunur_araligi_cozer() {
        // 84 px hücre, 100 kare, pencere [840, 1680) → kareler 10..=20.
        assert_eq!(visible_range(840.0, 1680.0, 84.0, 100), (10, 21));
        // Pencerenin solundaki kare (negatif min) kırpılır.
        assert_eq!(visible_range(-50.0, 100.0, 84.0, 100), (0, 3));
        // Sağ uç kare sayısını aşamaz.
        assert_eq!(visible_range(8300.0, 9999.0, 84.0, 100), (98, 100));
        // Boş oturum ve sıfır hücre güvenli.
        assert_eq!(visible_range(0.0, 500.0, 84.0, 0), (0, 0));
        assert_eq!(visible_range(0.0, 500.0, 0.0, 100), (0, 0));
    }

    #[test]
    fn kucuk_resim_boyutu_en_boyu_korur() {
        // Geniş kare genişlikten sınırlanır (76 px).
        assert_eq!(thumb_size(1920, 1080), (76, 43));
        // Uzun kare yükseklikten sınırlanır (en fazla 90 px).
        assert_eq!(thumb_size(100, 5000), (2, 90));
        // Küçük kare büyütülmez.
        assert_eq!(thumb_size(20, 10), (20, 10));
        // Sıfıra bölünme / sıfır boyut üretilmez.
        assert!(thumb_size(0, 0).0 >= 1 && thumb_size(0, 0).1 >= 1);
    }

    #[test]
    fn onbellek_dolunca_en_uzak_kareyi_atar() {
        let keys = [0usize, 1, 5, 9];
        // 1'e en uzak kare 9 (mesafe 8), 9'a en uzak 0 (mesafe 9).
        assert_eq!(eviction_victim(keys.iter(), 1), Some(9));
        assert_eq!(eviction_victim(keys.iter(), 9), Some(0));
        // Eşit uzaklıkta `max_by_key` sonuncuyu tutar; çökme/boş durum yok.
        assert_eq!(eviction_victim(keys.iter(), 5), Some(0));
        assert_eq!(eviction_victim([].iter(), 0), None);
    }

    #[test]
    fn gecikme_degisikligi_paket_tamponunu_paylasir() {
        // Önbellek anahtarı bu adres; paylaşılmasa her gecikme düzenlemesi
        // tüm küçük resimleri yeniden çözerdi.
        let a = frame(7, 40);
        let b = a.with_delay(120);
        assert_eq!(b.delay, 120);
        assert_eq!(a.packed().as_ptr(), b.packed().as_ptr());
    }

    #[test]
    fn kucuk_resim_onbellek_gecersizlesme_ve_yeniden_kullanim() {
        let ctx = egui::Context::default();
        let mut cache = ThumbCache::default();
        let a = frame(0, 40);
        let b = frame(0, 40);
        load_session(vec![a.clone(), b.clone()]);

        // İlk çözme önbelleğe yazar; doku küçültülmüş boyutta olmalı.
        let first = cache.get(&ctx, 1, 0, (4, 4)).expect("doku üretilmeli");
        assert_eq!(first.size_vec2(), egui::vec2(4.0, 4.0));
        assert_eq!(cache.entries.len(), 1);

        // Aynı kare (yalnız gecikmesi değişmiş olsa bile) yeniden çözülmez.
        let same = cache.get(&ctx, 1, 0, (4, 4)).expect("önbellekten gelmeli");
        assert_eq!(first.id(), same.id());
        assert_eq!(cache.entries.len(), 1);

        // Kare yeni piksel tamponuyla değişti: doku yenilenmeli.
        let replaced = frame(200, 40);
        assert_ne!(replaced.packed().as_ptr(), a.packed().as_ptr());
        load_session(vec![replaced, b]);
        let changed = cache.get(&ctx, 1, 0, (4, 4)).expect("doku üretilmeli");
        assert_ne!(first.id(), changed.id());
        assert_eq!(cache.entries.len(), 1);

        // Yeni oturum ve boyut değişimi tüm girdileri atar.
        assert!(cache.get(&ctx, 2, 0, (4, 4)).is_some());
        assert_eq!(cache.entries.len(), 1);
    }
}
