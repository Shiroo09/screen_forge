//! Presenter-mode rendering: the laser trail and the spotlight vignette.
//! Port of the draw paths in `ScreenForge/Presenter/PresenterRenderer.cs`.
//!
//! Both operations paint onto a full-screen BGRA buffer that the overlay window
//! composites over the live desktop. The laser is a soft, fading stroke; the
//! spotlight dims everything outside a radius around the cursor.

use crate::canvas::{Canvas, Rgba};

/// Draws a laser trail: a soft stroke whose alpha fades along the trail.
///
/// `points` are `(x, y, alpha)` — the alpha is the per-point fade from
/// `sf_overlay::presenter::LaserTrail::visible`. `width` is the stroke width.
pub fn draw_laser(canvas: &mut Canvas, points: &[(f32, f32, f32)], color: Rgba, width: f32) {
    if points.is_empty() {
        return;
    }

    let half = (width / 2.0).max(1.0);
    for pair in points.windows(2) {
        let (x0, y0, a0) = pair[0];
        let (x1, y1, a1) = pair[1];
        let steps = (((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt() / 1.0).ceil() as i32;
        let steps = steps.clamp(1, 512);

        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let x = x0 + (x1 - x0) * t;
            let y = y0 + (y1 - y0) * t;
            let alpha = a0 + (a1 - a0) * t;
            disc(canvas, x, y, half, color, alpha);
        }
    }

    // A single point renders as a dot.
    if points.len() == 1 {
        let (x, y, a) = points[0];
        disc(canvas, x, y, half, color, a);
    }
}

/// Filled antialiased disc, using `alpha` as the coverage multiplier.
fn disc(canvas: &mut Canvas, cx: f32, cy: f32, radius: f32, color: Rgba, alpha: f32) {
    let min_x = (cx - radius - 1.0).floor() as i32;
    let max_x = (cx + radius + 1.0).ceil() as i32;
    let min_y = (cy - radius - 1.0).floor() as i32;
    let max_y = (cy + radius + 1.0).ceil() as i32;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let d = (dx * dx + dy * dy).sqrt();
            let cov = (radius + 0.5 - d).clamp(0.0, 1.0) * alpha.clamp(0.0, 1.0);
            if cov > 0.0 {
                canvas.blend(x, y, color, cov);
            }
        }
    }
}

/// Spotlights a region: everything is darkened by `dim` (0..1) except a soft
/// circle of `radius` around `(cx, cy)`. Matches the presenter spotlight.
pub fn draw_spotlight(canvas: &mut Canvas, cx: f32, cy: f32, radius: f32, softness: f32, dim: f32) {
    let dim = dim.clamp(0.0, 1.0);
    if dim <= 0.0 {
        return;
    }

    let radius = radius.max(1.0);
    let softness = softness.max(1.0);
    let (w, h) = (canvas.width(), canvas.height());
    let dark = Rgba::new(0, 0, 0, (dim * 255.0) as u8);

    for y in 0..h {
        for x in 0..w {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let d = (dx * dx + dy * dy).sqrt();

            // Inside the radius: no dim. Beyond radius+softness: full dim. Between:
            // a smooth falloff.
            let coverage = if d <= radius {
                0.0
            } else if d >= radius + softness {
                dim
            } else {
                let t = (d - radius) / softness;
                // Smoothstep for a soft edge.
                let s = t * t * (3.0 - 2.0 * t);
                s * dim
            };

            if coverage > 0.0 {
                canvas.blend(x, y, dark, coverage);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn surface(w: i32, h: i32) -> Vec<u8> {
        let mut px = vec![0u8; (w * h * 4) as usize];
        for chunk in px.chunks_exact_mut(4) {
            chunk[0] = 200;
            chunk[1] = 200;
            chunk[2] = 200;
            chunk[3] = 255;
        }
        px
    }

    #[test]
    fn laser_marks_its_path() {
        let mut px = surface(64, 64);
        {
            let mut c = Canvas::new(&mut px, 64, 64);
            draw_laser(&mut c, &[(10.0, 32.0, 1.0), (54.0, 32.0, 1.0)], Rgba::rgb(0xFF3D6E), 6.0);
        }
        let c = Canvas::new(&mut px, 64, 64);
        // Somewhere along the trail is now red-tinted.
        let mid = c.get(32, 32).unwrap();
        assert!(mid.r > 200 && mid.g < 150, "trail pixel {mid:?}");
    }

    #[test]
    fn laser_alpha_scales_impact() {
        let mut full = surface(64, 64);
        let mut faint = surface(64, 64);
        {
            let mut c = Canvas::new(&mut full, 64, 64);
            draw_laser(&mut c, &[(10.0, 32.0, 1.0), (54.0, 32.0, 1.0)], Rgba::new(255, 0, 0, 255), 6.0);
            let mut c2 = Canvas::new(&mut faint, 64, 64);
            draw_laser(&mut c2, &[(10.0, 32.0, 0.2), (54.0, 32.0, 0.2)], Rgba::new(255, 0, 0, 255), 6.0);
        }
        let cf = Canvas::new(&mut full, 64, 64);
        let cn = Canvas::new(&mut faint, 64, 64);
        // The faint trail leaves the pixel closer to the background.
        assert!(cn.get(32, 32).unwrap().g > cf.get(32, 32).unwrap().g);
    }

    #[test]
    fn spotlight_dims_outside_keeps_centre() {
        let mut px = surface(80, 80);
        {
            let mut c = Canvas::new(&mut px, 80, 80);
            draw_spotlight(&mut c, 40.0, 40.0, 15.0, 8.0, 0.6);
        }
        let c = Canvas::new(&mut px, 80, 80);
        // Centre still bright.
        assert!(c.get(40, 40).unwrap().r > 180);
        // Corner darkened.
        assert!(c.get(2, 2).unwrap().r < 140);
    }

    #[test]
    fn spotlight_no_dim_is_noop() {
        let mut px = surface(32, 32);
        {
            let mut c = Canvas::new(&mut px, 32, 32);
            draw_spotlight(&mut c, 16.0, 16.0, 10.0, 5.0, 0.0);
        }
        let c = Canvas::new(&mut px, 32, 32);
        assert_eq!(c.get(2, 2).unwrap().r, 200);
    }
}
