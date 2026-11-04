//! Blur and pixelate, the redaction tools. Port of the masking transforms in
//! `ScreenForge/Editor/StepBlurImageItems.cs` (`BlurItem`).
//!
//! Both operators work on a BGRA buffer in place over a rectangle. Blur is a
//! separable box blur repeated for a gaussian-like falloff; pixelate averages
//! each `block`×`block` cell to one colour. The rectangle is clamped to bounds,
//! so an item dragged past the edge never reads out of range.

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MaskKind {
    /// Soft blur, strength = radius in pixels.
    #[default]
    Blur,
    /// Mosaic, strength = cell size in pixels.
    Pixelate,
}

/// Applies the mask to the BGRA buffer.
///
/// `rect` is `(x, y, w, h)` in pixels; `strength` is the blur radius or the
/// pixelate cell size (whichever `kind` selects). Out-of-bounds rects are clamped.
pub fn apply_mask(
    bgra: &mut [u8],
    width: i32,
    height: i32,
    rect: (i32, i32, i32, i32),
    kind: MaskKind,
    strength: f32,
) {
    if width <= 0 || height <= 0 || bgra.len() < (width * height * 4) as usize {
        return;
    }

    let (x, y, w, h) = clamp_rect(rect, width, height);
    if w < 1 || h < 1 {
        return;
    }

    match kind {
        MaskKind::Blur => box_blur(bgra, width, height, x, y, w, h, strength.max(0.0) as i32),
        MaskKind::Pixelate => pixelate(bgra, width, height, x, y, w, h, (strength.max(1.0)) as i32),
    }
}

fn clamp_rect(rect: (i32, i32, i32, i32), width: i32, height: i32) -> (i32, i32, i32, i32) {
    let x = rect.0.clamp(0, (width - 1).max(0));
    let y = rect.1.clamp(0, (height - 1).max(0));
    let w = rect.2.clamp(0, width - x);
    let h = rect.3.clamp(0, height - y);
    (x, y, w, h)
}

/// One box-blur pass over the rectangle, reading from `src` into `dst` so the
/// original rows are not half-blurred as we go.
#[allow(clippy::too_many_arguments)]
fn box_blur(
    bgra: &mut [u8],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    radius: i32,
) {
    if radius <= 0 {
        return;
    }
    let radius = radius.min(64); // bound the work; deeper blur is a repeat, not a huge kernel
    let radius = radius.min(w.max(h) / 2).max(0);
    if radius == 0 {
        return;
    }

    let (w_us, _h_us) = (width as usize, height as usize);
    let row_bytes = width as usize * 4;

    // Work on a copy of the rect so horizontal and vertical passes are independent.
    let mut region = vec![0u8; (w * h * 4) as usize];
    for row in 0..h as usize {
        let src = ((y as usize + row) * w_us + x as usize) * 4;
        let dst = row * w as usize * 4;
        region[dst..dst + w as usize * 4].copy_from_slice(&bgra[src..src + w as usize * 4]);
    }

    let mut temp = vec![0u8; region.len()];
    // Horizontal pass into temp.
    for row in 0..h as usize {
        for col in 0..w as usize {
            let mut sum = [0u32; 3];
            let mut count = 0u32;
            let lo = col.saturating_sub(radius as usize);
            let hi = (col + radius as usize + 1).min(w as usize);
            for c in lo..hi {
                let i = (row * w as usize + c) * 4;
                sum[0] += region[i] as u32;
                sum[1] += region[i + 1] as u32;
                sum[2] += region[i + 2] as u32;
                count += 1;
            }
            let o = (row * w as usize + col) * 4;
            temp[o] = (sum[0] / count) as u8;
            temp[o + 1] = (sum[1] / count) as u8;
            temp[o + 2] = (sum[2] / count) as u8;
            temp[o + 3] = 255;
        }
    }
    // Vertical pass back into region.
    for row in 0..h as usize {
        for col in 0..w as usize {
            let mut sum = [0u32; 3];
            let mut count = 0u32;
            let lo = row.saturating_sub(radius as usize);
            let hi = (row + radius as usize + 1).min(h as usize);
            for r in lo..hi {
                let i = (r * w as usize + col) * 4;
                sum[0] += temp[i] as u32;
                sum[1] += temp[i + 1] as u32;
                sum[2] += temp[i + 2] as u32;
                count += 1;
            }
            let o = (row * w as usize + col) * 4;
            region[o] = (sum[0] / count) as u8;
            region[o + 1] = (sum[1] / count) as u8;
            region[o + 2] = (sum[2] / count) as u8;
            region[o + 3] = 255;
        }
    }

    // Write back.
    for row in 0..h as usize {
        let dst = ((y as usize + row) * w_us + x as usize) * 4;
        let src = row * w as usize * 4;
        bgra[dst..dst + w as usize * 4].copy_from_slice(&region[src..src + w as usize * 4]);
    }
    let _ = row_bytes;
}

/// Averages each `cell`×`cell` block to a single colour.
#[allow(clippy::too_many_arguments, clippy::manual_checked_ops)]
fn pixelate(bgra: &mut [u8], width: i32, _height: i32, x: i32, y: i32, w: i32, h: i32, cell: i32) {
    if cell <= 1 {
        return;
    }
    let w_us = width as usize;

    let mut by = 0;
    while by < h {
        let mut bx = 0;
        while bx < w {
            let x0 = bx;
            let x1 = (bx + cell).min(w);
            let y0 = by;
            let y1 = (by + cell).min(h);

            let mut sum = [0u32; 3];
            let mut count = 0u32;
            for yy in y0..y1 {
                for xx in x0..x1 {
                    let i = ((y as usize + yy as usize) * w_us + (x as usize + xx as usize)) * 4;
                    sum[0] += bgra[i] as u32;
                    sum[1] += bgra[i + 1] as u32;
                    sum[2] += bgra[i + 2] as u32;
                    count += 1;
                }
            }
            // count is never 0 here: the cell is non-empty by construction.
            if count > 0 {
                let avg = [(sum[0] / count) as u8, (sum[1] / count) as u8, (sum[2] / count) as u8];
                for yy in y0..y1 {
                    for xx in x0..x1 {
                        let i = ((y as usize + yy as usize) * w_us + (x as usize + xx as usize)) * 4;
                        bgra[i] = avg[0];
                        bgra[i + 1] = avg[1];
                        bgra[i + 2] = avg[2];
                        bgra[i + 3] = 255;
                    }
                }
            }
            bx += cell;
        }
        by += cell;
    }
}
