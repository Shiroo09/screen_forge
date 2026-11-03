//! Animated GIF writer. Port of `ScreenForge/Gif/Encoder/GifFile.cs`
//! (adapted from ScreenToGif, Nicke Manarin).
//!
//! Usage: `set_canvas_size` -> (optional) `build_global_palette` -> `add_frame`
//! per frame -> `finish`. The writer is generic over `W: Write` so frames can be
//! streamed straight to a file: nothing accumulates in memory during export.

use std::io::Write;

use crate::lzw::LzwEncoder;
use crate::palette::PaletteMap;
use crate::quantize::{NeuralQuantizer, OctreeQuantizer, Quantizer};
use crate::{QuantizerType, Rgb};

const DISPOSAL_LEAVE: i32 = 1; // leave the previous frame in place (required for delta frames)

/// Frame rectangle, the Rust stand-in for `System.Windows.Int32Rect` carrying only
/// what the encoder needs.
#[derive(Clone, Copy, Debug, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub struct GifFile<W: Write> {
    stream: W,

    /// 0 = loop forever, -1 = no loop.
    pub repeat_count: i32,
    /// Max colors per frame (2-256).
    pub maximum_number_color: i32,
    pub quantizer_type: QuantizerType,
    /// Neural sampling factor: 1 = best quality, 20 = fastest.
    pub sampling_factor: i32,
    /// All frames share one palette; no palette repeats, smaller file.
    pub use_global_palette: bool,
    pub use_dithering: bool,

    header_written: bool,
    finished: bool,
    canvas_width: i32,
    canvas_height: i32,

    global_palette: Option<Vec<Rgb>>,
    global_map: Option<PaletteMap>,
    global_size_field: i32,
}

impl<W: Write> GifFile<W> {
    pub fn new(stream: W) -> Self {
        Self {
            stream,
            repeat_count: 0,
            maximum_number_color: 256,
            quantizer_type: QuantizerType::Neural,
            sampling_factor: 5,
            use_global_palette: false,
            use_dithering: false,
            header_written: false,
            finished: false,
            canvas_width: 0,
            canvas_height: 0,
            global_palette: None,
            global_map: None,
            global_size_field: 0,
        }
    }

    /// Logical screen size. Must be set before the first `add_frame`.
    pub fn set_canvas_size(&mut self, width: i32, height: i32) {
        self.canvas_width = width;
        self.canvas_height = height;
    }

    /// Builds one global palette from sample frames. Only meaningful when
    /// `use_global_palette` is on.
    pub fn build_global_palette(&mut self, samples: &[Vec<u8>]) {
        if !self.use_global_palette || samples.is_empty() {
            return;
        }

        let merged = merge_samples(samples);
        // Global palette delta frames need a transparency slot -> reserve one.
        let max_colors = (self.maximum_number_color.clamp(2, 256) - 1) as usize;
        let mut palette = self.build_palette(&merged, max_colors);
        palette.push(Rgb::new(0, 0, 0)); // transparent slot (last index)
        let transparent = palette.len() as i32 - 1;
        self.global_map = Some(PaletteMap::new(&palette, transparent));
        self.global_size_field = size_field(palette.len());
        self.global_palette = Some(palette);
    }

    /// Writes a frame. `bgra` is BGRA data of `rect` size. Pixels with alpha 0
    /// are treated as "unchanged" and written transparent.
    pub fn add_frame(&mut self, bgra: &[u8], rect: Rect, delay_ms: i32, has_transparency: bool) {
        if rect.width <= 0 || rect.height <= 0 {
            return;
        }

        let dithering = self.use_dithering;
        let use_global = self.use_global_palette && self.global_map.is_some();
        let global_size_field = self.global_size_field;

        let indexed: Vec<u8>;
        let size_field_value: i32;
        let transparent_index: i32;
        let mut local_palette: Option<Vec<Rgb>> = None;

        if use_global {
            let map = self.global_map.as_mut().expect("global map");
            let ti = map.transparent_index();
            indexed = if dithering {
                map_dithered(bgra, rect.width, rect.height, map, ti)
            } else {
                map_direct(bgra, map, ti)
            };
            size_field_value = global_size_field;
            transparent_index = ti;
        } else {
            let mut max_colors = self.maximum_number_color.clamp(2, 256) as usize;
            if has_transparency {
                max_colors -= 1;
            }

            let mut palette = self.build_palette(bgra, max_colors);
            let ti = if has_transparency {
                palette.push(Rgb::new(0, 0, 0));
                palette.len() as i32 - 1
            } else {
                -1
            };

            let mut map = PaletteMap::new(&palette, ti);
            indexed = if dithering {
                map_dithered(bgra, rect.width, rect.height, &mut map, ti)
            } else {
                map_direct(bgra, &mut map, ti)
            };
            size_field_value = size_field(palette.len());
            transparent_index = ti;
            local_palette = Some(palette);
        }

        if !self.header_written {
            self.write_header(size_field_value);
            self.header_written = true;
        }

        // C# keys the descriptor bit and the palette write on the *flag*
        // `UseGlobalPalette`, not on whether a global map actually exists. Keep
        // that: when the flag is set without a built palette both stay off.
        let write_local_table = !self.use_global_palette;

        self.write_graphic_control_extension(delay_ms, transparent_index);
        self.write_image_descriptor(rect, size_field_value, write_local_table);
        if write_local_table {
            if let Some(palette) = &local_palette {
                self.write_palette(palette, size_field_value);
            }
        }

        let mut lzw = LzwEncoder::new(&indexed, size_field_value + 1);
        lzw.encode(&mut self.stream);
    }

    // ─── Palette production ───────────────────────────────────────────────

    fn build_palette(&self, bgra: &[u8], max_colors: usize) -> Vec<Rgb> {
        let max_colors = max_colors.clamp(2, 256);

        let mut palette = match self.quantizer_type {
            QuantizerType::Octree => {
                let mut q = OctreeQuantizer::new(max_colors);
                q.quantize(bgra)
            }
            QuantizerType::Neural => {
                let mut q = NeuralQuantizer::new(
                    self.sampling_factor.clamp(1, 20),
                    max_colors,
                );
                q.quantize(bgra)
            }
        };

        if palette.is_empty() {
            palette.push(Rgb::new(0, 0, 0));
        }
        palette.truncate(max_colors);
        palette
    }

    // ─── Block writers ────────────────────────────────────────────────────

    fn write_header(&mut self, size_field_value: i32) {
        self.write_string("GIF89a");
        self.write_short(if self.canvas_width > 0 { self.canvas_width } else { 1 });
        self.write_short(if self.canvas_height > 0 { self.canvas_height } else { 1 });

        let global_table = self.use_global_palette && self.global_palette.is_some();
        // color resolution = 8 bit
        let packed = 0x70 | if global_table { 0x80 | (size_field_value & 0x07) } else { 0 };
        self.write_byte(packed);
        self.write_byte(0); // background color index
        self.write_byte(0); // pixel aspect ratio

        if global_table {
            let palette = self.global_palette.clone().expect("global palette");
            self.write_palette(&palette, size_field_value);
        }

        if self.repeat_count > -1 {
            self.write_application_extension();
        }
    }

    fn write_palette(&mut self, palette: &[Rgb], size_field_value: i32) {
        for color in palette {
            self.write_byte(color.r as i32);
            self.write_byte(color.g as i32);
            self.write_byte(color.b as i32);
        }

        let slots = 2usize << size_field_value;
        for _ in palette.len()..slots {
            self.write_byte(0);
            self.write_byte(0);
            self.write_byte(0);
        }
    }

    fn write_application_extension(&mut self) {
        self.write_byte(0x21);
        self.write_byte(0xff);
        self.write_byte(0x0b);
        self.write_string("NETSCAPE2.0");
        self.write_byte(0x03);
        self.write_byte(0x01);
        self.write_short(self.repeat_count);
        self.write_byte(0x00);
    }

    fn write_graphic_control_extension(&mut self, delay_ms: i32, transparent_index: i32) {
        self.write_byte(0x21);
        self.write_byte(0xf9);
        self.write_byte(0x04);

        // bits 4-2 disposal, bit 1 user input, bit 0 transparency
        let mut packed = (DISPOSAL_LEAVE & 0x07) << 2;
        if transparent_index >= 0 {
            packed |= 0x01;
        }
        self.write_byte(packed);

        // GIF delay is in 1/100 s; 0 degrades to "as fast as possible" in browsers.
        let centiseconds = ((delay_ms as f64) / 10.0).round() as i32;
        self.write_short(centiseconds.clamp(1, u16::MAX as i32));

        self.write_byte(if transparent_index >= 0 { transparent_index } else { 0 });
        self.write_byte(0);
    }

    fn write_image_descriptor(&mut self, rect: Rect, size_field_value: i32, write_local_table: bool) {
        self.write_byte(0x2c);
        self.write_short(rect.x);
        self.write_short(rect.y);
        self.write_short(rect.width);
        self.write_short(rect.height);

        // bit 7 local palette, bit 6 interlace, bit 5 sort, bits 2-0 palette size
        self.write_byte(if write_local_table {
            0x80 | (size_field_value & 0x07)
        } else {
            0
        });
    }

    // ─── Helpers ──────────────────────────────────────────────────────────

    fn write_byte(&mut self, value: i32) {
        let _ = self.stream.write_all(&[(value & 0xff) as u8]);
    }

    fn write_short(&mut self, value: i32) {
        let v = value as u16;
        let _ = self.stream.write_all(&[(v & 0xff) as u8, ((v >> 8) & 0xff) as u8]);
    }

    fn write_string(&mut self, value: &str) {
        let _ = self.stream.write_all(value.as_bytes());
    }

    /// Writes the trailer and flushes. Safe to call more than once.
    pub fn finish(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        self.write_byte(0x3b); // GIF trailer
        let _ = self.stream.flush();
    }
}

impl<W: Write> Drop for GifFile<W> {
    fn drop(&mut self) {
        self.finish();
    }
}

/// Palette size field: the smallest n (0-7) such that the slot count 2^(n+1) fits `count`.
pub fn size_field(count: usize) -> i32 {
    let mut n = 0;
    while n < 7 && (2usize << n) < count {
        n += 1;
    }
    n
}

/// Thins frames down into one evenly-sampled buffer for the global palette.
fn merge_samples(samples: &[Vec<u8>]) -> Vec<u8> {
    const MAX_SAMPLE_BYTES: usize = 8 * 1024 * 1024; // 2M pixels — plenty for a palette

    let total: usize = samples.iter().map(|s| s.len()).sum();
    if total <= MAX_SAMPLE_BYTES {
        let mut all = Vec::with_capacity(total);
        for s in samples {
            all.extend_from_slice(s);
        }
        return all;
    }

    let stride = total.div_ceil(MAX_SAMPLE_BYTES);
    let mut buffer = Vec::with_capacity(MAX_SAMPLE_BYTES);

    'outer: for s in samples {
        let mut i = 0;
        while i + 3 < s.len() {
            if buffer.len() + 3 >= MAX_SAMPLE_BYTES {
                break 'outer;
            }
            buffer.extend_from_slice(&s[i..i + 4]);
            i += 4 * stride;
        }
    }

    buffer
}

fn map_direct(bgra: &[u8], map: &mut PaletteMap, transparent_index: i32) -> Vec<u8> {
    let pixel_count = bgra.len() / 4;
    let mut output = vec![0u8; pixel_count];
    let transparent = transparent_index.max(0) as u8;

    let mut i = 0;
    for slot in output.iter_mut() {
        *slot = if transparent_index >= 0 && bgra[i + 3] == 0 {
            transparent
        } else {
            map.map(bgra[i + 2], bgra[i + 1], bgra[i])
        };
        i += 4;
    }

    output
}

/// Floyd-Steinberg dithering in a single pass. Error is carried in a two-row
/// rolling buffer, so memory is O(width).
fn map_dithered(
    bgra: &[u8],
    width: i32,
    height: i32,
    map: &mut PaletteMap,
    transparent_index: i32,
) -> Vec<u8> {
    let pixel_count = bgra.len() / 4;
    if width <= 0 || height <= 0 || width as usize * height as usize != pixel_count {
        return map_direct(bgra, map, transparent_index);
    }

    let width = width as usize;
    let height = height as usize;
    let mut output = vec![0u8; pixel_count];
    let transparent = transparent_index.max(0) as u8;

    // [row] holds R,G,B error per pixel; index 0 is a left sentinel.
    let mut curr = vec![0f32; (width + 2) * 3];
    let mut next = vec![0f32; (width + 2) * 3];

    for y in 0..height {
        next.iter_mut().for_each(|v| *v = 0.0);

        for x in 0..width {
            let p = y * width + x;
            let i = p * 4;

            if transparent_index >= 0 && bgra[i + 3] == 0 {
                output[p] = transparent;
                continue;
            }

            let e = (x + 1) * 3;
            let r = bgra[i + 2] as f32 + curr[e];
            let g = bgra[i + 1] as f32 + curr[e + 1];
            let b = bgra[i] as f32 + curr[e + 2];

            let (ri, gi, bi) = (clamp255(r) as u8, clamp255(g) as u8, clamp255(b) as u8);
            let index = map.find_nearest(ri, gi, bi);
            output[p] = index;

            let er = r - map.red_of(index) as f32;
            let eg = g - map.green_of(index) as f32;
            let eb = b - map.blue_of(index) as f32;

            spread(&mut curr, e + 3, er, eg, eb, 7.0 / 16.0);
            spread(&mut next, e - 3, er, eg, eb, 3.0 / 16.0);
            spread(&mut next, e, er, eg, eb, 5.0 / 16.0);
            spread(&mut next, e + 3, er, eg, eb, 1.0 / 16.0);
        }

        std::mem::swap(&mut curr, &mut next);
    }

    output
}

fn spread(buffer: &mut [f32], offset: usize, r: f32, g: f32, b: f32, factor: f32) {
    buffer[offset] += r * factor;
    buffer[offset + 1] += g * factor;
    buffer[offset + 2] += b * factor;
}

fn clamp255(value: f32) -> i32 {
    if value <= 0.0 {
        0
    } else if value >= 255.0 {
        255
    } else {
        value as i32
    }
}
