//! Compressed frame storage. Port of `ScreenForge/Gif/FrameStore.cs`.
//!
//! Raw BGRA frames are large (1920x1080 is 8.3 MB), so each frame is Deflate-
//! compressed on entry. `Compression::fast()` matches the C# `Fastest` choice:
//! `best()` compresses ~2x better but costs ~35 ms per frame and slows capture.

use flate2::write::{DeflateDecoder, DeflateEncoder};
use flate2::Compression;
use std::io::Write;

pub struct FrameStore {
    compressed: Vec<Vec<u8>>,
    frame_byte_count: usize,
    compressed_bytes: u64,
}

impl FrameStore {
    pub fn new(frame_byte_count: usize) -> Self {
        Self {
            compressed: Vec::new(),
            frame_byte_count,
            compressed_bytes: 0,
        }
    }

    pub fn count(&self) -> usize {
        self.compressed.len()
    }

    /// Memory used by the compressed frames.
    pub fn compressed_bytes(&self) -> u64 {
        self.compressed_bytes
    }

    /// Memory the frames would use uncompressed.
    pub fn raw_bytes(&self) -> u64 {
        self.compressed.len() as u64 * self.frame_byte_count as u64
    }

    pub fn ratio(&self) -> f64 {
        if self.compressed_bytes == 0 {
            1.0
        } else {
            self.raw_bytes() as f64 / self.compressed_bytes as f64
        }
    }

    /// Compresses and stores a frame, returning its compressed size.
    pub fn add(&mut self, pixels: &[u8]) -> u64 {
        let packed = compress(pixels);
        self.compressed_bytes += packed.len() as u64;
        let len = packed.len() as u64;
        self.compressed.push(packed);
        len
    }

    pub fn get(&self, index: usize) -> Vec<u8> {
        let mut out = vec![0u8; self.frame_byte_count];
        decompress_into(&self.compressed[index], &mut out);
        out
    }

    /// Undoes the last `add`. Needed because the memory limit is only known once
    /// the frame is compressed.
    pub fn remove_last(&mut self, stored_bytes: u64) {
        if self.compressed.pop().is_some() {
            self.compressed_bytes = self.compressed_bytes.saturating_sub(stored_bytes);
        }
    }

    /// Decompresses every frame and empties the store.
    pub fn drain_all(&mut self) -> Vec<Vec<u8>> {
        let packed = self.drain_packed();
        packed
            .iter()
            .map(|block| {
                let mut out = vec![0u8; self.frame_byte_count];
                decompress_into(block, &mut out);
                out
            })
            .collect()
    }

    /// Hands over the compressed frames without decompressing.
    pub fn drain_packed(&mut self) -> Vec<Vec<u8>> {
        self.compressed_bytes = 0;
        std::mem::take(&mut self.compressed)
    }

    pub fn clear(&mut self) {
        self.compressed.clear();
        self.compressed_bytes = 0;
    }
}

pub fn compress(pixels: &[u8]) -> Vec<u8> {
    let mut enc = DeflateEncoder::new(Vec::new(), Compression::fast());
    // Writing to a Vec cannot fail.
    enc.write_all(pixels).expect("deflate to Vec");
    enc.finish().expect("deflate finish")
}

/// Fills `dest` from a raw-Deflate block.
///
/// # Panics
/// Panics if the block is corrupt or shorter than `dest`. The C# original lets
/// `DeflateStream` throw `InvalidDataException` here; silently returning a
/// half-zero buffer would hide frame corruption in a lossless path.
pub fn decompress_into(packed: &[u8], dest: &mut [u8]) {
    let mut dec = DeflateDecoder::new(Vec::new());
    dec.write_all(packed).expect("corrupt deflate block");
    let out = dec.finish().expect("corrupt deflate block");
    assert!(
        out.len() >= dest.len(),
        "deflate block produced {} bytes, expected {}",
        out.len(),
        dest.len()
    );
    dest.copy_from_slice(&out[..dest.len()]);
}
