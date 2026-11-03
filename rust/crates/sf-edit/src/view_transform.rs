//! Canvas pixels ↔ scene coordinates. Port of `ScreenForge/Editor/CanvasViewTransform.cs`.
//!
//! Paint and mouse use the same formula; a DpiScale × stale-scale mismatch used to
//! land cropping in the middle of the screen at high DPI.

/// How the scene is laid out in the viewport.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LayoutMode {
    OneToOne,
    Fit,
}

/// A 2D point, the stand-in for `SkiaSharp.SKPoint`.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Computes the scale and offset for the given layout.
pub fn compute(
    layout: LayoutMode,
    scene_w: f32,
    scene_h: f32,
    px_w: i32,
    px_h: i32,
    dpi: f32,
) -> (f32, Point) {
    if scene_w <= 0.0 || scene_h <= 0.0 {
        return (1.0, Point::new(0.0, 0.0));
    }

    if layout == LayoutMode::OneToOne {
        let mut s = if px_w > 0 { px_w as f32 / scene_w } else { 1.0 };
        if s <= 0.0 || s.is_nan() {
            s = if dpi > 0.0 { dpi } else { 1.0 };
        }
        return (s, Point::new(0.0, 0.0));
    }

    let margin = 24.0 * dpi.max(1.0);
    let avail_w = px_w as f32 - margin * 2.0;
    let avail_h = px_h as f32 - margin * 2.0;
    let mut scale = (avail_w / scene_w).min(avail_h / scene_h);
    scale = scale.min(4.0);
    if scale <= 0.0 || scale.is_nan() {
        scale = 1.0;
    }
    let ox = (px_w as f32 - scene_w * scale) / 2.0;
    let oy = (px_h as f32 - scene_h * scale) / 2.0;
    (scale, Point::new(ox, oy))
}

/// Widget (DIP) coordinates → scene coordinates.
#[allow(clippy::too_many_arguments)]
pub fn wpf_to_scene(
    x: f64,
    y: f64,
    actual_w: f64,
    actual_h: f64,
    paint_px_w: i32,
    paint_px_h: i32,
    scale: f32,
    offset: Point,
) -> Point {
    if actual_w <= 0.0 || actual_h <= 0.0 || paint_px_w <= 0 || paint_px_h <= 0 || scale <= 0.0 {
        return Point::new(x as f32, y as f32);
    }

    let px = (x * paint_px_w as f64 / actual_w) as f32;
    let py = (y * paint_px_h as f64 / actual_h) as f32;
    Point::new((px - offset.x) / scale, (py - offset.y) / scale)
}

/// Scene coordinates → widget (DIP) coordinates.
#[allow(clippy::too_many_arguments)]
pub fn scene_to_wpf(
    scene_x: f32,
    scene_y: f32,
    actual_w: f64,
    actual_h: f64,
    paint_px_w: i32,
    paint_px_h: i32,
    scale: f32,
    offset: Point,
) -> (f64, f64) {
    if actual_w <= 0.0 || actual_h <= 0.0 || paint_px_w <= 0 || paint_px_h <= 0 {
        return (scene_x as f64, scene_y as f64);
    }

    let px = scene_x * scale + offset.x;
    let py = scene_y * scale + offset.y;
    (
        px as f64 * actual_w / paint_px_w as f64,
        py as f64 * actual_h / paint_px_h as f64,
    )
}
