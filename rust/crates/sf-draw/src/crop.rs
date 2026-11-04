//! Crop a BGRA buffer to a rectangle. Pure buffer math, no canvas needed.

/// Clamps `(x, y, w, h)` to `[0, width) × [0, height)`.
pub fn clamp_rect(rect: (i32, i32, i32, i32), w: i32, h: i32) -> (i32, i32, i32, i32) {
    if w <= 0 || h <= 0 {
        return (0, 0, 0, 0);
    }
    let x = rect.0.clamp(0, w - 1);
    let y = rect.1.clamp(0, h - 1);
    let cw = rect.2.clamp(0, w - x);
    let ch = rect.3.clamp(0, h - y);
    (x, y, cw, ch)
}

/// Crops a BGRA buffer to `rect`, returning the new buffer and its size.
pub fn crop_bgra(bgra: &[u8], w: i32, h: i32, rect: (i32, i32, i32, i32)) -> (Vec<u8>, i32, i32) {
    if w <= 0 || h <= 0 || bgra.len() < (w * h * 4) as usize {
        return (Vec::new(), 0, 0);
    }
    let (x, y, cw, ch) = clamp_rect(rect, w, h);
    if cw <= 0 || ch <= 0 {
        return (Vec::new(), 0, 0);
    }

    let stride = (w * 4) as usize;
    let row_bytes = (cw * 4) as usize;
    let mut out = Vec::with_capacity(row_bytes * ch as usize);
    for row in y..y + ch {
        let start = row as usize * stride + x as usize * 4;
        out.extend_from_slice(&bgra[start..start + row_bytes]);
    }
    (out, cw, ch)
}
