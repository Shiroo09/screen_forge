//! Pixel transforms over frames. Port of the pure parts of
//! `ScreenForge/Gif/Editing/ImageOperations.cs`.
//!
//! `Resize` in C# used GDI+ `HighQualityBicubic`; a matching bicubic resampler
//! lives here so behaviour is identical without a graphics stack.

use std::sync::Arc;

use crate::frame::EditorFrame;

/// Rotation direction for [`rotate_pixels`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RotateDirection {
    Left90,
    Right90,
    Half180,
}

/// Integer rectangle, the stand-in for `System.Windows.Int32Rect`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Crops every frame to `rect`.
pub fn crop(
    frames: &[Arc<EditorFrame>],
    width: i32,
    height: i32,
    rect: Rect,
) -> Vec<Arc<EditorFrame>> {
    let rect = clamp_rect(rect, width, height);
    if rect.width <= 0 || rect.height <= 0 {
        return frames.to_vec();
    }

    frames
        .iter()
        .map(|f| f.with_pixels(&crop_pixels(&f.pixels(), width, rect)))
        .collect()
}

/// Rotates every frame. 90° turns swap width and height.
pub fn rotate(
    frames: &[Arc<EditorFrame>],
    width: i32,
    height: i32,
    direction: RotateDirection,
) -> Vec<Arc<EditorFrame>> {
    frames
        .iter()
        .map(|f| f.with_pixels(&rotate_pixels(&f.pixels(), width, height, direction)))
        .collect()
}

/// Resizes every frame.
pub fn resize(
    frames: &[Arc<EditorFrame>],
    width: i32,
    height: i32,
    new_width: i32,
    new_height: i32,
) -> Vec<Arc<EditorFrame>> {
    if new_width <= 0 || new_height <= 0 || (new_width == width && new_height == height) {
        return frames.to_vec();
    }

    frames
        .iter()
        .map(|f| {
            f.with_pixels(&resize_pixels(&f.pixels(), width, height, new_width, new_height))
        })
        .collect()
}

/// The size after rotation.
pub fn size_after_rotate(width: i32, height: i32, direction: RotateDirection) -> (i32, i32) {
    if direction == RotateDirection::Half180 {
        (width, height)
    } else {
        (height, width)
    }
}

/// Maps an on-screen selection rectangle to source pixel coordinates.
/// Returns `None` when the selection is smaller than `minimum_size`.
#[allow(clippy::too_many_arguments)]
pub fn screen_rect_to_source(
    left: f64,
    top: f64,
    width: f64,
    height: f64,
    zoom: f64,
    source_width: i32,
    source_height: i32,
    minimum_size: i32,
) -> Option<Rect> {
    if zoom <= 0.0 || width <= 0.0 || height <= 0.0 {
        return None;
    }

    let rect = clamp_rect(
        Rect::new(
            (left / zoom).round() as i32,
            (top / zoom).round() as i32,
            (width / zoom).round() as i32,
            (height / zoom).round() as i32,
        ),
        source_width,
        source_height,
    );

    if rect.width < minimum_size || rect.height < minimum_size {
        None
    } else {
        Some(rect)
    }
}

/// Clamps a rectangle to the frame bounds. Port of `ClampRect`.
pub fn clamp_rect(rect: Rect, width: i32, height: i32) -> Rect {
    let x = rect.x.clamp(0, (width - 1).max(0));
    let y = rect.y.clamp(0, (height - 1).max(0));
    let w = rect.width.clamp(0, width - x);
    let h = rect.height.clamp(0, height - y);
    Rect::new(x, y, w, h)
}

/// Crop one BGRA buffer to `rect` (BGRA rows, 4 bytes per pixel).
pub fn crop_pixels(source: &[u8], source_width: i32, rect: Rect) -> Vec<u8> {
    let row_bytes = rect.width as usize * 4;
    let mut output = vec![0u8; rect.width as usize * rect.height as usize * 4];

    for y in 0..rect.height as usize {
        let source_offset = ((rect.y as usize + y) * source_width as usize + rect.x as usize) * 4;
        let dest_offset = y * row_bytes;
        output[dest_offset..dest_offset + row_bytes]
            .copy_from_slice(&source[source_offset..source_offset + row_bytes]);
    }

    output
}

/// Rotate one BGRA buffer. Output has the same byte length; only the layout changes.
pub fn rotate_pixels(source: &[u8], width: i32, height: i32, direction: RotateDirection) -> Vec<u8> {
    let (w, h) = (width as usize, height as usize);
    let src: Vec<[u8; 4]> = source.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect();
    let mut dst = vec![[0u8; 4]; w * h];

    match direction {
        RotateDirection::Right90 => {
            // (x,y) -> (height-1-y, x); new width = old height
            for y in 0..h {
                for x in 0..w {
                    dst[x * h + (h - 1 - y)] = src[y * w + x];
                }
            }
        }
        RotateDirection::Left90 => {
            // (x,y) -> (y, width-1-x)
            for y in 0..h {
                for x in 0..w {
                    dst[(w - 1 - x) * h + y] = src[y * w + x];
                }
            }
        }
        RotateDirection::Half180 => {
            let total = w * h;
            for i in 0..total {
                dst[total - 1 - i] = src[i];
            }
        }
    }

    dst.into_iter().flatten().collect()
}

/// Bicubic resize of one BGRA buffer. All output pixels are forced opaque
/// (the GIF path requires it), matching the C# tail loop.
pub fn resize_pixels(
    source: &[u8],
    width: i32,
    height: i32,
    new_width: i32,
    new_height: i32,
) -> Vec<u8> {
    let (sw, sh) = (width as usize, height as usize);
    let (dw, dh) = (new_width as usize, new_height as usize);
    let mut output = vec![0u8; dw * dh * 4];

    let x_ratio = sw as f64 / dw as f64;
    let y_ratio = sh as f64 / dh as f64;

    for dy in 0..dh {
        // Sample at pixel centres, as GDI+ does.
        let sy = (dy as f64 + 0.5) * y_ratio - 0.5;
        for dx in 0..dw {
            let sx = (dx as f64 + 0.5) * x_ratio - 0.5;
            let (b, g, r) = bicubic_sample(source, sw, sh, sx, sy);
            let o = (dy * dw + dx) * 4;
            output[o] = b;
            output[o + 1] = g;
            output[o + 2] = r;
            output[o + 3] = 255;
        }
    }

    output
}

const BICUBIC_A: f64 = -0.5;

fn cubic(p0: f64, p1: f64, p2: f64, p3: f64, t: f64) -> f64 {
    // Catmull-Rom style cubic convolution (the "HighQualityBicubic" family).
    let a = BICUBIC_A;
    p0 * (a * t * t * t - 2.0 * a * t * t + a * t)
        + p1 * ((a + 2.0) * t * t * t - (a + 3.0) * t * t + 1.0)
        + p2 * ((a + 2.0) * t * t * t + (2.0 * a + 3.0) * t * t - a * t)
        + p3 * (a * t * t * t - a * t * t)
}

fn channel_at(source: &[u8], sw: usize, sh: usize, x: i64, y: i64, channel: usize) -> f64 {
    let x = x.clamp(0, sw as i64 - 1);
    let y = y.clamp(0, sh as i64 - 1);
    source[(y as usize * sw + x as usize) * 4 + channel] as f64
}

fn bicubic_sample(source: &[u8], sw: usize, sh: usize, sx: f64, sy: f64) -> (u8, u8, u8) {
    let x0 = sx.floor() as i64;
    let y0 = sy.floor() as i64;
    let tx = sx - x0 as f64;
    let ty = sy - y0 as f64;

    let mut out = [0u8; 3];
    for (channel, slot) in out.iter_mut().enumerate() {
        let mut rows = [0f64; 4];
        for (row, r) in rows.iter_mut().enumerate() {
            let yy = y0 - 1 + row as i64;
            let p0 = channel_at(source, sw, sh, x0 - 1, yy, channel);
            let p1 = channel_at(source, sw, sh, x0, yy, channel);
            let p2 = channel_at(source, sw, sh, x0 + 1, yy, channel);
            let p3 = channel_at(source, sw, sh, x0 + 2, yy, channel);
            *r = cubic(p0, p1, p2, p3, tx);
        }
        let value = cubic(rows[0], rows[1], rows[2], rows[3], ty);
        *slot = value.round().clamp(0.0, 255.0) as u8;
    }

    (out[0], out[1], out[2])
}
