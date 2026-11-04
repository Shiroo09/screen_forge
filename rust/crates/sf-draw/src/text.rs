//! Text annotation with a built-in bitmap font.
//!
//! The C# text tool used WPF's font stack. A full font shaper is out of scope for
//! a headless rasterizer, so this ships a compact 5×7 uppercase/digit font — enough
//! for labels and step captions — with a scale factor. Text is upscaled by integer
//! or fractional scale so it reads clearly on any screenshot size.

use crate::canvas::{Canvas, Rgba};
use crate::shapes::Stroke;

/// A 5×7 glyph: 7 rows, each a 5-bit mask (bit 4 = leftmost).
type Glyph = [u8; 7];

/// Returns the glyph for an ASCII character, or a blank for unknown ones.
fn glyph(c: char) -> Glyph {
    let u = c.to_ascii_uppercase();
    match u {
        ' ' => [0, 0, 0, 0, 0, 0, 0],
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111],
        'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
        'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
        '.' => [0, 0, 0, 0, 0, 0, 0b00100],
        ',' => [0, 0, 0, 0, 0, 0b00100, 0b01000],
        '!' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0, 0b00100],
        '?' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0, 0b00100],
        ':' => [0, 0b00100, 0, 0, 0b00100, 0, 0],
        '-' => [0, 0, 0, 0b11111, 0, 0, 0],
        '_' => [0, 0, 0, 0, 0, 0, 0b11111],
        '/' => [0b00001, 0b00010, 0b00100, 0b00100, 0b01000, 0b10000, 0b00000],
        _ => [0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111], // unknown: box
    }
}

/// Width in pixels of `text` at the given scale (5 px glyph + 1 px gap).
pub fn measure(text: &str, scale: f32) -> (f32, f32) {
    let n = text.chars().count() as f32;
    let w = if n == 0.0 { 0.0 } else { (n * 6.0 - 1.0) * scale };
    (w, 7.0 * scale)
}

/// Draws `text` with its top-left at `(x, y)`. A background ribbon (padding +
/// fill) is drawn first when `ribbon` is set, matching the C# text item's option.
pub fn draw_text(canvas: &mut Canvas, x: f32, y: f32, text: &str, scale: f32, color: Rgba, ribbon: Option<Rgba>) {
    let scale = scale.max(0.5);
    let (tw, th) = measure(text, scale);

    if let Some(bg) = ribbon {
        let pad = 3.0 * scale;
        let rect = Stroke::new(bg, 1.0);
        crate::shapes::draw_rect(canvas, x - pad, y - pad, tw + pad * 2.0, th + pad * 2.0, rect.with_fill(bg));
    }

    let mut cursor = x;
    for ch in text.chars() {
        let g = glyph(ch);
        for (row, bits) in g.iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) != 0 {
                    let px = cursor + col as f32 * scale;
                    let py = y + row as f32 * scale;
                    fill_block(canvas, px, py, scale, color);
                }
            }
        }
        cursor += 6.0 * scale;
    }
}

/// Fills a `scale`×`scale` block, blending edge pixels so fractional scale stays
/// antialiased.
fn fill_block(canvas: &mut Canvas, x: f32, y: f32, scale: f32, color: Rgba) {
    let x1 = x + scale;
    let y1 = y + scale;
    let min_x = x.floor() as i32;
    let max_x = x1.ceil() as i32;
    let min_y = y.floor() as i32;
    let max_y = y1.ceil() as i32;

    for py in min_y..max_y {
        for px in min_x..max_x {
            let cx = overlap(px as f32, x, x1);
            let cy = overlap(py as f32, y, y1);
            if cx > 0.0 && cy > 0.0 {
                canvas.blend(px, py, color, cx * cy);
            }
        }
    }
}

fn overlap(p: f32, lo: f32, hi: f32) -> f32 {
    let left = p.max(lo);
    let right = (p + 1.0).min(hi);
    (right - left).clamp(0.0, 1.0)
}
