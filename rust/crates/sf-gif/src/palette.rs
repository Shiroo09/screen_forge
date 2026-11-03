//! Nearest-color lookup. Port of `ScreenForge/Gif/Encoder/PaletteMap.cs`.
//!
//! RGB is reduced to 5-5-5 bits and memoized in a 32768-entry cache, turning the
//! per-pixel O(palette) scan into O(1) after the first hit.

use crate::Rgb;

const CACHE_BITS: u32 = 5;
const CACHE_SIZE: usize = 1 << (CACHE_BITS * 3); // 32768

pub struct PaletteMap {
    reds: Vec<u8>,
    greens: Vec<u8>,
    blues: Vec<u8>,
    cache: Vec<u8>,
    cached: Vec<bool>,
    transparent_index: i32,
}

impl PaletteMap {
    /// `transparent_index` is ignored (treated as none) when out of range.
    pub fn new(palette: &[Rgb], transparent_index: i32) -> Self {
        let count = palette.len();
        let mut reds = Vec::with_capacity(count);
        let mut greens = Vec::with_capacity(count);
        let mut blues = Vec::with_capacity(count);

        for c in palette {
            reds.push(c.r);
            greens.push(c.g);
            blues.push(c.b);
        }

        let transparent_index = if transparent_index >= 0 && (transparent_index as usize) < count {
            transparent_index
        } else {
            -1
        };

        Self {
            reds,
            greens,
            blues,
            cache: vec![0; CACHE_SIZE],
            cached: vec![false; CACHE_SIZE],
            transparent_index,
        }
    }

    pub fn transparent_index(&self) -> i32 {
        self.transparent_index
    }

    /// Nearest palette index for a full RGB value (cached, 5-5-5 quantized).
    pub fn map(&mut self, r: u8, g: u8, b: u8) -> u8 {
        let key = (((r as usize) >> 3) << 10) | (((g as usize) >> 3) << 5) | ((b as usize) >> 3);
        if self.cached[key] {
            return self.cache[key];
        }

        let index = self.find_nearest(r, g, b);
        self.cache[key] = index;
        self.cached[key] = true;
        index
    }

    /// Uncached exact search — for error-diffusion streams like dithering.
    pub fn find_nearest(&self, r: u8, g: u8, b: u8) -> u8 {
        let (r, g, b) = (r as i32, g as i32, b as i32);
        let mut best = 0usize;
        let mut best_dist = i32::MAX;

        for i in 0..self.reds.len() {
            if i as i32 == self.transparent_index {
                continue;
            }

            let dr = r - self.reds[i] as i32;
            let dg = g - self.greens[i] as i32;
            let db = b - self.blues[i] as i32;
            let dist = dr * dr + dg * dg + db * db;

            if dist >= best_dist {
                continue;
            }

            best_dist = dist;
            best = i;
            if dist == 0 {
                break;
            }
        }

        best as u8
    }

    pub fn red_of(&self, index: u8) -> u8 {
        self.reds[index as usize]
    }

    pub fn green_of(&self, index: u8) -> u8 {
        self.greens[index as usize]
    }

    pub fn blue_of(&self, index: u8) -> u8 {
        self.blues[index as usize]
    }
}
