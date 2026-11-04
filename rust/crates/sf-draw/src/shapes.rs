//! Shape rasterizers drawn onto a [`Canvas`].
//!
//! Antialiasing is done by supersampling the distance field: for each pixel the
//! distance to the shape is computed and coverage ramps over one pixel. That is
//! slower than Skia's analytic coverage but produces the same visual result and
//! keeps the crate dependency-free.

use crate::canvas::{Canvas, Rgba};

/// Line/stroke style shared by the shape helpers.
#[derive(Clone, Copy, Debug)]
pub struct Stroke {
    pub color: Rgba,
    pub width: f32,
    /// When set, the interior of a rect/ellipse is filled with this first.
    pub fill: Option<Rgba>,
}

impl Stroke {
    pub fn new(color: Rgba, width: f32) -> Self {
        Self {
            color,
            width,
            fill: None,
        }
    }

    pub fn with_fill(mut self, fill: Rgba) -> Self {
        self.fill = Some(fill);
        self
    }
}

/// Antialiased line segment from `(x0,y0)` to `(x1,y1)`.
pub fn draw_stroke(canvas: &mut Canvas, x0: f32, y0: f32, x1: f32, y1: f32, stroke: Stroke) {
    let half = (stroke.width / 2.0).max(0.5);
    let min_x = (x0.min(x1) - half - 1.0).floor() as i32;
    let max_x = (x0.max(x1) + half + 1.0).ceil() as i32;
    let min_y = (y0.min(y1) - half - 1.0).floor() as i32;
    let max_y = (y0.max(y1) + half + 1.0).ceil() as i32;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let d = distance_to_segment(px, py, x0, y0, x1, y1);
            let coverage = (half + 0.5 - d).clamp(0.0, 1.0);
            canvas.blend(x, y, stroke.color, coverage);
        }
    }
}

/// Freehand polyline through `points` (the pen tool).
pub fn draw_stroke_path(canvas: &mut Canvas, points: &[(f32, f32)], stroke: Stroke) {
    for pair in points.windows(2) {
        draw_stroke(canvas, pair[0].0, pair[0].1, pair[1].0, pair[1].1, stroke);
    }
    // A single point renders as a dot.
    if points.len() == 1 {
        draw_stroke(canvas, points[0].0, points[0].1, points[0].0, points[0].1, stroke);
    }
}

/// Antialiased axis-aligned rectangle outline (optionally filled).
pub fn draw_rect(canvas: &mut Canvas, x: f32, y: f32, w: f32, h: f32, stroke: Stroke) {
    if let Some(fill) = stroke.fill {
        fill_rect(canvas, x, y, w, h, fill);
    }

    let (x1, y1) = (x + w, y + h);
    draw_stroke(canvas, x, y, x1, y, stroke);
    draw_stroke(canvas, x1, y, x1, y1, stroke);
    draw_stroke(canvas, x1, y1, x, y1, stroke);
    draw_stroke(canvas, x, y1, x, y, stroke);
}

fn fill_rect(canvas: &mut Canvas, x: f32, y: f32, w: f32, h: f32, color: Rgba) {
    let min_x = x.floor().max(0.0) as i32;
    let max_x = (x + w).ceil() as i32;
    let min_y = y.floor().max(0.0) as i32;
    let max_y = (y + h).ceil() as i32;

    for py in min_y..max_y {
        for px in min_x..max_x {
            // Coverage by the fraction of the pixel inside the rect.
            let cx = coverage_1d(px as f32, x, x + w);
            let cy = coverage_1d(py as f32, y, y + h);
            canvas.blend(px, py, color, cx * cy);
        }
    }
}

/// Fraction of the unit pixel at `p` covered by the interval `[lo, hi]`.
fn coverage_1d(p: f32, lo: f32, hi: f32) -> f32 {
    let left = p.max(lo);
    let right = (p + 1.0).min(hi);
    (right - left).clamp(0.0, 1.0)
}

/// Antialiased ellipse outline inscribed in the rect (optionally filled).
pub fn draw_ellipse(canvas: &mut Canvas, x: f32, y: f32, w: f32, h: f32, stroke: Stroke) {
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    let (rx, ry) = ((w / 2.0).max(0.5), (h / 2.0).max(0.5));
    let half = (stroke.width / 2.0).max(0.5);

    let min_x = (cx - rx - half - 1.0).floor() as i32;
    let max_x = (cx + rx + half + 1.0).ceil() as i32;
    let min_y = (cy - ry - half - 1.0).floor() as i32;
    let max_y = (cy + ry + half + 1.0).ceil() as i32;

    for py in min_y..=max_y {
        for px in min_x..=max_x {
            let (dx, dy) = (px as f32 + 0.5 - cx, py as f32 + 0.5 - cy);
            // Normalized radius; 1.0 is the edge.
            let r = ((dx / rx).powi(2) + (dy / ry).powi(2)).sqrt();

            if let Some(fill) = stroke.fill {
                let inside = (1.0 - r) * rx.min(ry); // approx distance inward
                let cov = (inside + 0.5).clamp(0.0, 1.0);
                canvas.blend(px, py, fill, cov);
            }

            // Stroke: distance from the ellipse boundary, in pixels.
            let boundary = (r - 1.0).abs() * rx.min(ry);
            let cov = (half + 0.5 - boundary).clamp(0.0, 1.0);
            if cov > 0.0 {
                canvas.blend(px, py, stroke.color, cov);
            }
        }
    }
}

/// Antialiased arrow: a line from tail to tip plus a two-bar head, matching the
/// C# `ArrowItem` (head length ~4× line width, 25° half-angle).
pub fn draw_arrow(canvas: &mut Canvas, x0: f32, y0: f32, x1: f32, y1: f32, stroke: Stroke) {
    draw_stroke(canvas, x0, y0, x1, y1, stroke);

    let dx = x1 - x0;
    let dy = y1 - y0;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1.0 {
        return;
    }

    let ux = dx / len;
    let uy = dy / len;
    let head = (stroke.width * 4.0).max(8.0);
    let half_angle = 25f32.to_radians();

    for sign in [-1.0f32, 1.0] {
        let angle = half_angle * sign;
        let (ca, sa) = (angle.cos(), angle.sin());
        // Rotate the reverse direction by ±half_angle.
        let bx = -ux * ca + uy * sa;
        let by = -ux * sa - uy * ca;
        draw_stroke(canvas, x1, y1, x1 + bx * head, y1 + by * head, stroke);
    }
}

/// A filled circle with a number glyph made of strokes — the numbered-step tool.
///
/// The digit is drawn with a 7-segment style so no font is needed; matches the
/// C# `StepItem` look (circle badge, white numeral) closely enough for annotation.
pub fn draw_step(canvas: &mut Canvas, cx: f32, cy: f32, radius: f32, number: u32, badge: Rgba, digit: Rgba) {
    // Filled circle.
    let fill = Stroke::new(badge, 1.0);
    let min_x = (cx - radius - 1.0).floor() as i32;
    let max_x = (cx + radius + 1.0).ceil() as i32;
    let min_y = (cy - radius - 1.0).floor() as i32;
    let max_y = (cy + radius + 1.0).ceil() as i32;
    for py in min_y..=max_y {
        for px in min_x..=max_x {
            let (dx, dy) = (px as f32 + 0.5 - cx, py as f32 + 0.5 - cy);
            let d = (dx * dx + dy * dy).sqrt();
            let cov = (radius + 0.5 - d).clamp(0.0, 1.0);
            canvas.blend(px, py, fill.color, cov);
        }
    }

    // Seven-segment digit.
    let seg = Stroke::new(digit, (radius * 0.18).max(1.0));
    draw_digit(canvas, cx, cy, radius * 0.62, number, seg);
}

/// Draws `number` using seven segments inside a box of half-size `s` centred at
/// `(cx, cy)`. Handles 0-9; larger numbers fall back to the last digit.
fn draw_digit(canvas: &mut Canvas, cx: f32, cy: f32, s: f32, number: u32, seg: Stroke) {
    // Segment layout: a top, b top-right, c bottom-right, d bottom, e bottom-left,
    // f top-left, g middle.
    let set = segments_for(number % 10);
    let l = cx - s;
    let r = cx + s;
    let t = cy - s;
    let m = cy;
    let b = cy + s;

    if set[0] {
        draw_stroke(canvas, l, t, r, t, seg); // a
    }
    if set[1] {
        draw_stroke(canvas, r, t, r, m, seg); // b
    }
    if set[2] {
        draw_stroke(canvas, r, m, r, b, seg); // c
    }
    if set[3] {
        draw_stroke(canvas, l, b, r, b, seg); // d
    }
    if set[4] {
        draw_stroke(canvas, l, m, l, b, seg); // e
    }
    if set[5] {
        draw_stroke(canvas, l, t, l, m, seg); // f
    }
    if set[6] {
        draw_stroke(canvas, l, m, r, m, seg); // g
    }
}

fn segments_for(digit: u32) -> [bool; 7] {
    // a, b, c, d, e, f, g
    match digit {
        0 => [true, true, true, true, true, true, false],
        1 => [false, true, true, false, false, false, false],
        2 => [true, true, false, true, true, false, true],
        3 => [true, true, true, true, false, false, true],
        4 => [false, true, true, false, false, true, true],
        5 => [true, false, true, true, false, true, true],
        6 => [true, false, true, true, true, true, true],
        7 => [true, true, true, false, false, false, false],
        8 => [true, true, true, true, true, true, true],
        _ => [true, true, true, true, false, true, true], // 9
    }
}

/// Distance from point `(x, y)` to segment `(ax,ay)`–`(bx,by)`.
pub fn distance_to_segment(x: f32, y: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let (dx, dy) = (bx - ax, by - ay);
    let len_sq = dx * dx + dy * dy;
    if len_sq <= f32::EPSILON {
        return ((x - ax).powi(2) + (y - ay).powi(2)).sqrt();
    }
    let t = (((x - ax) * dx + (y - ay) * dy) / len_sq).clamp(0.0, 1.0);
    let (px, py) = (ax + t * dx, ay + t * dy);
    ((x - px).powi(2) + (y - py).powi(2)).sqrt()
}
