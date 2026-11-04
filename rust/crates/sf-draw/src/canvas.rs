//! A BGRA drawing surface with antialiased coverage blending.

/// An RGBA color with straight alpha, the annotation equivalent of `SKColor`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// A fully opaque color from a packed 0xRRGGBB value.
    pub const fn rgb(rgb: u32) -> Self {
        Self::new((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255)
    }
}

/// A drawing surface over a BGRA buffer (width × height, 4 bytes per pixel).
pub struct Canvas<'a> {
    pixels: &'a mut [u8],
    width: i32,
    height: i32,
}

impl<'a> Canvas<'a> {
    /// Wraps a BGRA buffer. `pixels` must be at least `width * height * 4` bytes.
    pub fn new(pixels: &'a mut [u8], width: i32, height: i32) -> Self {
        Self {
            pixels,
            width,
            height,
        }
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    /// Blends `color` into the pixel at `(x, y)` with extra `coverage` (0..1),
    /// which is what antialiasing modulates.
    pub fn blend(&mut self, x: i32, y: i32, color: Rgba, coverage: f32) {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return;
        }
        let cov = coverage.clamp(0.0, 1.0) * (color.a as f32 / 255.0);
        if cov <= 0.0 {
            return;
        }

        let i = ((y * self.width + x) * 4) as usize;
        // BGRA storage, RGBA color: channel order is swapped on read/write.
        let dst_b = self.pixels[i] as f32;
        let dst_g = self.pixels[i + 1] as f32;
        let dst_r = self.pixels[i + 2] as f32;

        self.pixels[i] = lerp(dst_b, color.b as f32, cov);
        self.pixels[i + 1] = lerp(dst_g, color.g as f32, cov);
        self.pixels[i + 2] = lerp(dst_r, color.r as f32, cov);
        // Annotation is composited onto an opaque screenshot.
        self.pixels[i + 3] = 255;
    }

    /// Applies a blur/pixelate mask over a rectangle in this canvas.
    pub fn apply_mask(&mut self, rect: (i32, i32, i32, i32), kind: crate::blur::MaskKind, strength: f32) {
        crate::blur::apply_mask(self.pixels, self.width, self.height, rect, kind, strength);
    }

    /// Reads a pixel back as RGBA. Test helper and blur source.
    pub fn get(&self, x: i32, y: i32) -> Option<Rgba> {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return None;
        }
        let i = ((y * self.width + x) * 4) as usize;
        Some(Rgba::new(
            self.pixels[i + 2],
            self.pixels[i + 1],
            self.pixels[i],
            self.pixels[i + 3],
        ))
    }

}

fn lerp(dst: f32, src: f32, t: f32) -> u8 {
    (dst + (src - dst) * t).round().clamp(0.0, 255.0) as u8
}
