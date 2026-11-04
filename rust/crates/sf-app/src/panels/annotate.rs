//! Annotate panel ("Düzenle"): draw shapes on the captured image, then export.
//!
//! Shapes are accumulated in `app.shapes` and can be undone/cleared. The canvas
//! shows a live preview: the source BGRA is copied, the committed shapes plus the
//! in-progress drag are drawn, and the result is uploaded as a texture. Export
//! flattens the shapes onto a fresh copy and saves via `rfd`.

use crate::app::ScreenForgeApp;
use crate::theme;
use egui::{Color32, RichText, Sense};
use sf_draw::canvas::Canvas;
use sf_draw::model::{Shape, Tool};
use sf_draw::Rgba;

/// Everything the toolbar can do.
const TOOLS: [Tool; 8] = [
    Tool::Rect,
    Tool::Ellipse,
    Tool::Arrow,
    Tool::Line,
    Tool::Pen,
    Tool::Step,
    Tool::Blur,
    Tool::Text,
];

const PALETTE: [u32; 6] = [0xEA6F12, 0xE5484D, 0x2563EB, 0x16A34A, 0xFFFFFF, 0x111111];

pub fn ui(app: &mut ScreenForgeApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.heading("Düzenle");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(&app.status).color(theme::TEXT_DIM).size(12.0));
        });
    });
    ui.label(
        RichText::new("Yakalanan görüntüye şekil çiz, sonra kaydet.")
            .color(theme::TEXT_DIM),
    );
    ui.add_space(8.0);

    toolbar(app, ui);
    ui.add_space(8.0);

    if app.captured.is_none() {
        empty_state(ui);
        return;
    }

    canvas(app, ui);
}

fn toolbar(app: &mut ScreenForgeApp, ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(theme::SURFACE)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .inner_margin(egui::Margin::same(10))
        .corner_radius(8.0)
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for tool in TOOLS {
                    let selected = app.tool == tool;
                    let label = tool_label(tool);
                    let btn = egui::Button::new(
                        RichText::new(label)
                            .size(13.0)
                            .color(if selected { Color32::WHITE } else { theme::TEXT }),
                    )
                    .fill(if selected { theme::ACCENT } else { theme::SURFACE_HI })
                    .min_size(egui::vec2(64.0, 30.0));
                    if ui.add(btn).clicked() {
                        app.tool = tool;
                    }
                }

                ui.separator();

                for rgb in PALETTE {
                    let selected = app.color == Rgba::rgb(rgb);
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(26.0, 26.0), Sense::click());
                    let c = Color32::from_rgb(
                        ((rgb >> 16) & 0xFF) as u8,
                        ((rgb >> 8) & 0xFF) as u8,
                        (rgb & 0xFF) as u8,
                    );
                    ui.painter().rect_filled(rect, 6.0, c);
                    if selected {
                        ui.painter().rect_stroke(
                            rect,
                            6.0,
                            egui::Stroke::new(2.0, theme::ACCENT),
                            egui::StrokeKind::Outside,
                        );
                    }
                    if resp.clicked() {
                        app.color = Rgba::rgb(rgb);
                    }
                }

                ui.separator();

                ui.label("Kalınlık");
                ui.add(
                    egui::Slider::new(&mut app.annotate.line_width, 1.0..=24.0)
                        .fixed_decimals(1),
                );
            });

            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if app.tool == Tool::Blur {
                    ui.label("Bulanıklık");
                    ui.add(egui::DragValue::new(&mut app.annotate.blur_strength).range(1.0..=40.0));
                }
                if app.tool == Tool::Text {
                    ui.label("Metin");
                    ui.add(egui::TextEdit::singleline(&mut app.annotate.text).desired_width(180.0));
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Kaydet").clicked() {
                        export(app);
                    }
                    if ui.button("Temizle").clicked() {
                        app.shapes.clear();
                    }
                    if ui.button("Geri al").clicked() {
                        app.shapes.pop();
                    }
                    ui.label(
                        RichText::new(format!("{} şekil", app.shapes.len()))
                            .color(theme::TEXT_DIM)
                            .size(12.0),
                    );
                });
            });
        });
}

fn tool_label(tool: Tool) -> &'static str {
    match tool {
        Tool::Rect => "Kutu",
        Tool::Ellipse => "Elips",
        Tool::Arrow => "Ok",
        Tool::Line => "Çizgi",
        Tool::Pen => "Kalem",
        Tool::Step => "Adım",
        Tool::Blur => "Bulanık",
        Tool::Text => "Metin",
    }
}

fn empty_state(ui: &mut egui::Ui) {
    ui.add_space(30.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new("Önce bir görüntü yakala").color(theme::TEXT).size(16.0));
        ui.label(
            RichText::new("Yakala sekmesinden ekran görüntüsü al, sonra burada düzenle")
                .color(theme::TEXT_DIM)
                .size(12.0),
        );
    });
}

/// The image canvas: shows the source with shapes applied, and captures drags.
fn canvas(app: &mut ScreenForgeApp, ui: &mut egui::Ui) {
    // Build the preview: source copy + committed shapes + live drag shape.
    let (src_w, src_h) = {
        let img = app.captured.as_ref().unwrap();
        (img.width, img.height)
    };

    let avail = ui.available_size();
    let scale = (avail.x / src_w as f32).min(avail.y / src_h as f32).min(1.0);
    let disp = egui::vec2(src_w as f32 * scale, src_h as f32 * scale);

    let (rect, response) = ui.allocate_exact_size(disp, Sense::drag());
    let origin = rect.min;

    // The preview texture is cached in the panel, keyed by a signature of the
    // shapes, so a full BGRA copy + re-render + upload happens only when the
    // drawing actually changes — not every frame.
    let sig = shape_signature(&app.shapes);
    let tex = preview_texture(ui.ctx(), app, src_w, src_h, sig);
    ui.painter().image(
        tex.id(),
        rect,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );

    // Drag handling, in image coordinates.
    let to_image = |p: egui::Pos2| ((p.x - origin.x) / scale, (p.y - origin.y) / scale);

    if response.drag_started() {
        if let Some(p) = response.interact_pointer_pos() {
            app.drag_from = Some(to_image(p));
        }
    }

    if response.dragged() {
        if let Some(p) = response.interact_pointer_pos() {
            let to = to_image(p);
            if let Some(from) = app.drag_from {
                let live = live_shape(app, from, to);
                paint_live(ui, origin, scale, app.annotate.line_width, &live);
            }
        }
    }

    if response.drag_stopped() {
        if let (Some(from), Some(p)) = (app.drag_from, response.interact_pointer_pos()) {
            let to = to_image(p);
            let shape = finalize_shape(app, from, to);
            app.shapes.push(shape);
        }
        app.drag_from = None;
    }

    ui.add_space(4.0);
    ui.label(
        RichText::new(format!("{src_w} × {src_h} px  ·  ölçek {:.0}%", scale * 100.0))
            .color(theme::TEXT_DIM)
            .size(11.0),
    );
}

/// Preview RGBA with committed shapes rendered on a copy of the source.
fn build_preview(app: &ScreenForgeApp, w: i32, h: i32) -> Vec<u8> {
    let src = &app.captured.as_ref().unwrap().bgra;
    let mut bgra = src.clone();
    {
        let mut c = Canvas::new(&mut bgra, w, h);
        sf_draw::model::render_all(&mut c, &app.shapes);
    }
    to_rgba(&bgra)
}

/// A cheap signature of the shape list: enough to detect any edit.
fn shape_signature(shapes: &[Shape]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    shapes.len().hash(&mut h);
    for s in shapes {
        // Debug formatting is stable enough for a change-detection key.
        format!("{s:?}").hash(&mut h);
    }
    h.finish()
}

/// The cached preview texture, rebuilt only when the shape signature changes.
///
/// Cached in a panel-local slot rather than on `ImageBuf`, so the Capture tab's
/// texture keeps showing the raw capture and the annotated preview never leaks
/// back into the source image.
fn preview_texture(
    ctx: &egui::Context,
    app: &ScreenForgeApp,
    w: i32,
    h: i32,
    sig: u64,
) -> egui::TextureHandle {
    use std::sync::{Mutex, OnceLock};

    struct Cache {
        sig: u64,
        size: (i32, i32),
        tex: Option<egui::TextureHandle>,
    }
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();

    let cache = CACHE.get_or_init(|| {
        Mutex::new(Cache {
            sig: u64::MAX,
            size: (0, 0),
            tex: None,
        })
    });

    let mut guard = cache.lock().unwrap();
    let stale = guard.sig != sig || guard.size != (w, h) || guard.tex.is_none();
    if stale {
        let preview = build_preview(app, w, h);
        let img = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &preview);
        guard.tex = Some(ctx.load_texture("annotate-preview", img, egui::TextureOptions::LINEAR));
        guard.sig = sig;
        guard.size = (w, h);
    }
    guard.tex.clone().unwrap()
}

/// Draws the in-progress shape straight onto the canvas as an overlay, so every
/// tool previews before the drag is released.
fn paint_live(ui: &egui::Ui, origin: egui::Pos2, scale: f32, width: f32, shape: &Shape) {
    let p = ui.painter();
    let to_screen = |x: f32, y: f32| origin + egui::vec2(x * scale, y * scale);
    let stroke = egui::Stroke::new((width * scale).max(1.0), theme::ACCENT);

    match shape {
        Shape::Rect { x, y, w, h, .. } => {
            let r = egui::Rect::from_min_size(to_screen(*x, *y), egui::vec2(*w * scale, *h * scale));
            p.rect_stroke(r, 0.0, stroke, egui::StrokeKind::Middle);
        }
        Shape::Ellipse { x, y, w, h, .. } => {
            let r = egui::Rect::from_min_size(to_screen(*x, *y), egui::vec2(*w * scale, *h * scale));
            p.add(egui::Shape::ellipse_stroke(r.center(), r.size() / 2.0, stroke));
        }
        Shape::Arrow { x0, y0, x1, y1, .. } => {
            let (a, b) = (to_screen(*x0, *y0), to_screen(*x1, *y1));
            p.line_segment([a, b], stroke);
            let d = b - a;
            let len = d.length();
            if len > 1.0 {
                let dir = d / len;
                let head = (len * 0.28).clamp(6.0, 18.0);
                let (c, s) = (30f32.to_radians().cos(), 30f32.to_radians().sin());
                let left = b - egui::vec2(dir.x * c - dir.y * s, dir.x * s + dir.y * c) * head;
                let right = b - egui::vec2(dir.x * c + dir.y * s, -dir.x * s + dir.y * c) * head;
                p.line_segment([b, left], stroke);
                p.line_segment([b, right], stroke);
            }
        }
        Shape::Line { x0, y0, x1, y1, .. } => {
            p.line_segment([to_screen(*x0, *y0), to_screen(*x1, *y1)], stroke);
        }
        Shape::Pen { points, .. } => {
            let pts: Vec<egui::Pos2> = points.iter().map(|(x, y)| to_screen(*x, *y)).collect();
            if pts.len() >= 2 {
                p.add(egui::Shape::line(pts, stroke));
            }
        }
        Shape::Step { cx, cy, radius, number, color } => {
            let c = to_screen(*cx, *cy);
            let rad = (*radius * scale).max(3.0);
            p.circle_filled(c, rad, color32(*color));
            p.circle_stroke(c, rad, egui::Stroke::new(1.5, theme::ACCENT));
            p.text(
                c,
                egui::Align2::CENTER_CENTER,
                number.to_string(),
                egui::FontId::proportional(rad * 1.2),
                Color32::WHITE,
            );
        }
        Shape::Blur { x, y, w, h, .. } => {
            let r = egui::Rect::from_min_size(to_screen(*x, *y), egui::vec2(*w * scale, *h * scale));
            p.rect_filled(r, 2.0, Color32::from_rgba_unmultiplied(0x10, 0x12, 0x18, 160));
            p.rect_stroke(r, 2.0, stroke, egui::StrokeKind::Inside);
            let pts = [r.left_top(), r.right_bottom()];
            p.extend(egui::Shape::dashed_line(pts.as_slice(), stroke, 4.0, 4.0));
        }
        Shape::Text { x, y, text, scale: ts, color, .. } => {
            let size = (7.0 * ts * scale).max(9.0);
            p.text(
                to_screen(*x, *y),
                egui::Align2::LEFT_TOP,
                text,
                egui::FontId::proportional(size),
                color32(*color),
            );
        }
    }
}

fn color32(c: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

/// Converts a completed drag into a shape for the active tool.
fn finalize_shape(app: &ScreenForgeApp, from: (f32, f32), to: (f32, f32)) -> Shape {
    let w = app.annotate.line_width;
    build_shape(app, from, to, w)
}

/// Converts an in-progress drag into a preview shape (same geometry as finalize).
fn live_shape(app: &ScreenForgeApp, from: (f32, f32), to: (f32, f32)) -> Shape {
    build_shape(app, from, to, app.annotate.line_width)
}

fn build_shape(app: &ScreenForgeApp, from: (f32, f32), to: (f32, f32), _w: f32) -> Shape {
    let (x0, y0) = from;
    let (x1, y1) = to;
    let (rx, ry) = (x0.min(x1), y0.min(y1));
    let (rw, rh) = ((x1 - x0).abs(), (y1 - y0).abs());

    match app.tool {
        Tool::Rect => Shape::Rect { x: rx, y: ry, w: rw, h: rh, color: app.color },
        Tool::Ellipse => Shape::Ellipse { x: rx, y: ry, w: rw, h: rh, color: app.color },
        Tool::Arrow => Shape::Arrow { x0, y0, x1, y1, color: app.color },
        Tool::Line => Shape::Line { x0, y0, x1, y1, color: app.color },
        Tool::Pen => Shape::Pen { points: vec![from, to], color: app.color },
        Tool::Step => Shape::Step {
            cx: x1,
            cy: y1,
            radius: 18.0,
            number: (app.shapes.len() + 1) as u32,
            color: app.color,
        },
        Tool::Blur => Shape::Blur {
            x: rx,
            y: ry,
            w: rw,
            h: rh,
            kind: sf_draw::blur::MaskKind::Blur,
            strength: app.annotate.blur_strength,
        },
        Tool::Text => Shape::Text {
            x: x1,
            y: y1,
            text: app.annotate.text.clone(),
            scale: 2.0,
            color: app.color,
            ribbon: true,
        },
    }
}

/// Flattens shapes onto a fresh copy of the source and saves it.
fn export(app: &mut ScreenForgeApp) {
    let Some(img) = app.captured.as_ref() else {
        app.notify("önce bir görüntü yakala");
        return;
    };
    let (w, h) = (img.width, img.height);
    let mut bgra = img.bgra.clone();
    {
        let mut c = Canvas::new(&mut bgra, w, h);
        sf_draw::model::render_all(&mut c, &app.shapes);
    }

    let format = app.settings.output_format;
    let ext = match format {
        sf_settings::ImageFormat::Png => "png",
        sf_settings::ImageFormat::Jpeg => "jpg",
        sf_settings::ImageFormat::Webp => "webp",
    };
    let mut dialog = rfd::FileDialog::new().set_file_name(format!("ScreenForge-duzenlenmis.{ext}"));
    if !app.settings.save_directory.trim().is_empty() {
        dialog = dialog.set_directory(&app.settings.save_directory);
    }
    let Some(path) = dialog.save_file() else {
        return;
    };

    let rgba = to_rgba(&bgra);
    let result = image::RgbaImage::from_raw(w as u32, h as u32, rgba)
        .ok_or_else(|| "görüntü boyutu uyuşmadı".to_string())
        .and_then(|image| match format {
            sf_settings::ImageFormat::Png => image.save(&path).map_err(|e| e.to_string()),
            sf_settings::ImageFormat::Jpeg => {
                let rgb = image::DynamicImage::ImageRgba8(image).to_rgb8();
                rgb.save(&path).map_err(|e| e.to_string())
            }
            sf_settings::ImageFormat::Webp => {
                image.save(&path).map_err(|e| e.to_string())
            }
        });

    match result {
        Ok(()) => {
            let shown = path.display().to_string();
            app.status = format!("kaydedildi: {shown}");
            app.notify(format!("kaydedildi: {shown}"));
        }
        Err(e) => app.notify(format!("hata: {e}")),
    }
}

/// BGRA → RGBA for egui/image.
fn to_rgba(bgra: &[u8]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(bgra.len());
    for px in bgra.chunks_exact(4) {
        rgba.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
    }
    rgba
}
