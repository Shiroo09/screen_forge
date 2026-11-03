//! GIF pipeline for ScreenForge, ported from `ScreenForge/Gif`.
//!
//! The C# original held decoded frames as `byte[]` BGRA buffers pinned by an
//! undo snapshot stack, which is where the multi-gigabyte peaks came from. This
//! port keeps the same on-disk format and the same palette/LZW output, but the
//! writer is pull-based: frames are handed in one at a time and never accumulated.

pub mod frame_store;
pub mod gif_file;
pub mod lzw;
pub mod palette;
pub mod quantize;

/// 24-bit color, the Rust replacement for `System.Windows.Media.Color`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

/// Which palette quantizer to use. Mirrors `ScreenForge.Gif.Encoder.QuantizerType`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum QuantizerType {
    #[default]
    Neural,
    Octree,
}
