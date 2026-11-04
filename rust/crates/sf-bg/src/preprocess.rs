//! u2netp input packing and mask resizing. Port of `LocalSubjectSegmenter.PackInput`
//! and `ResizeMask`.

/// The model's fixed square input size.
pub const INPUT_SIZE: usize = 320;

/// Packs a BGRA image into the model's NCHW float input.
///
/// Steps, matching the C# `PackInput`: bilinear-resize to 320×320, divide by the
/// image's max channel value, then normalize each channel with the ImageNet
/// mean/std and write as planar RGB (R plane, then G, then B).
pub fn pack_input(bgra: &[u8], width: i32, height: i32) -> Vec<f32> {
    let resized = resize_bgra(bgra, width, height, INPUT_SIZE as i32, INPUT_SIZE as i32);

    // Max channel value across the resized image (at least 1 to avoid /0).
    let mut max = 1u8;
    for px in resized.chunks_exact(4) {
        max = max.max(px[0]).max(px[1]).max(px[2]);
    }
    let inv = 1.0f32 / max as f32;

    let plane = INPUT_SIZE * INPUT_SIZE;
    let mut data = vec![0f32; 3 * plane];
    for p in 0..plane {
        let i = p * 4;
        let b = resized[i] as f32 * inv;
        let g = resized[i + 1] as f32 * inv;
        let r = resized[i + 2] as f32 * inv;
        data[p] = (r - 0.485) / 0.229;
        data[plane + p] = (g - 0.456) / 0.224;
        data[plane * 2 + p] = (b - 0.406) / 0.225;
    }
    data
}

/// Bilinear BGRA resize, matching the C# `SKSamplingOptions(Linear, None)`.
fn resize_bgra(src: &[u8], sw: i32, sh: i32, dw: i32, dh: i32) -> Vec<u8> {
    let mut dst = vec![0u8; (dw * dh * 4) as usize];
    if sw <= 0 || sh <= 0 || src.len() < (sw * sh * 4) as usize {
        return dst;
    }

    let x_ratio = sw as f32 / dw as f32;
    let y_ratio = sh as f32 / dh as f32;

    for y in 0..dh {
        let sy = (y as f32 + 0.5) * y_ratio - 0.5;
        let y0 = sy.floor().max(0.0) as i32;
        let y1 = (y0 + 1).min(sh - 1);
        let fy = (sy - y0 as f32).clamp(0.0, 1.0);

        for x in 0..dw {
            let sx = (x as f32 + 0.5) * x_ratio - 0.5;
            let x0 = sx.floor().max(0.0) as i32;
            let x1 = (x0 + 1).min(sw - 1);
            let fx = (sx - x0 as f32).clamp(0.0, 1.0);

            let o = ((y * dw + x) * 4) as usize;
            for c in 0..4usize {
                let p00 = src[((y0 * sw + x0) as usize) * 4 + c] as f32;
                let p10 = src[((y0 * sw + x1) as usize) * 4 + c] as f32;
                let p01 = src[((y1 * sw + x0) as usize) * 4 + c] as f32;
                let p11 = src[((y1 * sw + x1) as usize) * 4 + c] as f32;
                let top = p00 + (p10 - p00) * fx;
                let bottom = p01 + (p11 - p01) * fx;
                dst[o + c] = (top + (bottom - top) * fy).round().clamp(0.0, 255.0) as u8;
            }
        }
    }

    dst
}

/// Converts the raw model output into an 8-bit mask sized `dw`×`dh`.
///
/// The C# `ResizeMask` min-max normalizes the output then scales to 0..255. The
/// interpolation here is nearest-neighbour to keep the mapping deterministic.
pub fn resize_mask(src: &[f32], sw: i32, sh: i32, dw: i32, dh: i32) -> Vec<u8> {
    let n = (sw * sh) as usize;
    if n == 0 || src.len() < n {
        return vec![0u8; (dw * dh).max(0) as usize];
    }

    let mut min = src[0];
    let mut max = src[0];
    for &v in &src[..n] {
        min = min.min(v);
        max = max.max(v);
    }
    let range = (max - min).max(1e-6);

    let mut out = vec![0u8; (dw * dh) as usize];
    for y in 0..dh {
        let sy = (y * sh / dh).clamp(0, sh - 1);
        for x in 0..dw {
            let sx = (x * sw / dw).clamp(0, sw - 1);
            let norm = (src[(sy * sw + sx) as usize] - min) / range;
            out[(y * dw + x) as usize] = (norm * 255.0).clamp(0.0, 255.0) as u8;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(w: i32, h: i32) -> Vec<u8> {
        let mut px = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                px[i] = (x * 255 / w.max(1)) as u8; // B
                px[i + 1] = (y * 255 / h.max(1)) as u8; // G
                px[i + 2] = ((x + y) * 127 / (w + h).max(1)) as u8; // R
                px[i + 3] = 255;
            }
        }
        px
    }

    #[test]
    fn pack_input_has_expected_shape() {
        let px = gradient(100, 60);
        let data = pack_input(&px, 100, 60);
        assert_eq!(data.len(), 3 * INPUT_SIZE * INPUT_SIZE);
        // Normalized values are finite and within a sane range.
        assert!(data.iter().all(|v| v.is_finite()));
        let r_plane = &data[0..INPUT_SIZE * INPUT_SIZE];
        assert!(r_plane.iter().any(|&v| v != 0.0), "packing should not be all zero");
    }

    #[test]
    fn resize_mask_normalizes_to_full_range() {
        // A 2x2 output from a 2x2 input covering 0.0..1.0.
        let src = [0.0f32, 0.5, 1.0, 0.25];
        let mask = resize_mask(&src, 2, 2, 2, 2);
        assert_eq!(mask[0], 0); // min
        assert_eq!(mask[2], 255); // max
        assert!(mask[1] > 0 && mask[1] < 255);
    }

    #[test]
    fn resize_mask_handles_flat_input() {
        // All-equal input must not divide by zero.
        let src = [0.7f32; 4];
        let mask = resize_mask(&src, 2, 2, 2, 2);
        assert_eq!(mask.len(), 4);
    }

    #[test]
    fn resize_bgra_keeps_dimensions() {
        let px = gradient(64, 48);
        let resized = resize_bgra(&px, 64, 48, 320, 320);
        assert_eq!(resized.len(), 320 * 320 * 4);
    }
}
