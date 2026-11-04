//! Translucent, thick marker stroke — the C# `HighlightItem`.

use crate::canvas::{Canvas, Rgba};
use crate::shapes::{self, Stroke};

/// Marker ink never exceeds this alpha, so a highlight always tints the pixels
/// under it instead of covering them (C# `HighlightItem` uses alpha 120).
pub const HIGHLIGHT_ALPHA: u8 = 110;

/// Draws a thick, translucent band through `points`.
pub fn draw_highlight(canvas: &mut Canvas, points: &[(f32, f32)], color: Rgba, width: f32) {
    let ink = Rgba::new(color.r, color.g, color.b, color.a.min(HIGHLIGHT_ALPHA));
    shapes::draw_stroke_path(canvas, points, Stroke::new(ink, width.max(1.0)));
}
