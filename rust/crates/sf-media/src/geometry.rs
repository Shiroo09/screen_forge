//! Video frame size helpers. Port of `ScreenForge/Record/VideoGeometry.cs`.

pub const MAX_LONG_EDGE: i32 = 2560;

/// Rounds both dimensions down to even. Returns (0, 0) if either would drop
/// below 2 — encoders reject odd or degenerate dimensions.
pub fn even_size(width: i32, height: i32) -> (i32, i32) {
    let w = width & !1;
    let h = height & !1;
    if w < 2 || h < 2 {
        return (0, 0);
    }
    (w, h)
}

/// Caps the longest edge at `max_edge`, preserving aspect ratio, keeping even
/// dimensions. `max_edge` defaults to [`MAX_LONG_EDGE`].
pub fn cap_long_edge(width: i32, height: i32, max_edge: i32) -> (i32, i32) {
    let (w, h) = even_size(width, height);
    if w == 0 {
        return (0, 0);
    }
    let longest = w.max(h);
    if longest <= max_edge {
        return (w, h);
    }
    let scale = max_edge as f64 / longest as f64;
    even_size(
        (w as f64 * scale).round() as i32,
        (h as f64 * scale).round() as i32,
    )
}
