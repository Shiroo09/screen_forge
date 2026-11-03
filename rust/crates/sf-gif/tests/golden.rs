//! Byte-for-byte comparison against fixtures produced by the real C# encoder.
//!
//! The fixtures under `tests/fixtures/` are written by
//! `ScreenForge.Tests/GoldenGifFixture.cs` (run with `SF_WRITE_GOLDEN=1`).
//! Everything here — the pixel generator, the frame contents, the config — must
//! match that file exactly. A mismatch means the Rust port and the C# original
//! diverge, which is the one thing this port exists to avoid.

use sf_gif::gif_file::{GifFile, Rect};
use sf_gif::QuantizerType;

const W: usize = 32;
const H: usize = 24;

/// Produces the same bytes as the Rust writer for a given configuration.
fn encode(config: impl FnOnce(&mut GifFile<&mut Vec<u8>>)) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut gif = GifFile::new(&mut out);
        config(&mut gif);
    }
    out
}

fn compare(name: &str, actual: &[u8]) {
    let path = format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name);
    let expected = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("read fixture {path}: {e} — run the C# generator with SF_WRITE_GOLDEN=1"));

    if expected != actual {
        let first_diff = expected
            .iter()
            .zip(actual.iter())
            .position(|(a, b)| a != b)
            .unwrap_or(expected.len().min(actual.len()));
        panic!(
            "{name} diverges from the C# encoder: expected {} bytes, got {} bytes, first difference at offset {first_diff} \
             (expected 0x{:02x}, got 0x{:02x})",
            expected.len(),
            actual.len(),
            expected.get(first_diff).copied().unwrap_or(0),
            actual.get(first_diff).copied().unwrap_or(0),
        );
    }
}

fn add_frames(gif: &mut GifFile<&mut Vec<u8>>, frames: &[Vec<u8>]) {
    gif.set_canvas_size(W as i32, H as i32);
    for f in frames {
        gif.add_frame(f, Rect { x: 0, y: 0, width: W as i32, height: H as i32 }, 100, false);
    }
}

#[test]
fn neural_local_palette_matches_csharp() {
    let frames = frames();
    let bytes = encode(|gif| {
        gif.quantizer_type = QuantizerType::Neural;
        gif.sampling_factor = 5;
        gif.maximum_number_color = 256;
        gif.use_global_palette = false;
        gif.use_dithering = false;
        add_frames(gif, &frames);
    });
    compare("golden_neural_local.gif", &bytes);
}

#[test]
fn octree_local_palette_matches_csharp() {
    let frames = frames();
    let bytes = encode(|gif| {
        gif.quantizer_type = QuantizerType::Octree;
        gif.maximum_number_color = 64;
        gif.use_global_palette = false;
        gif.use_dithering = false;
        add_frames(gif, &frames);
    });
    compare("golden_octree_local.gif", &bytes);
}

#[test]
fn neural_dithering_matches_csharp() {
    let frames = frames();
    let bytes = encode(|gif| {
        gif.quantizer_type = QuantizerType::Neural;
        gif.sampling_factor = 5;
        gif.maximum_number_color = 32;
        gif.use_global_palette = false;
        gif.use_dithering = true;
        add_frames(gif, &frames);
    });
    compare("golden_neural_dither.gif", &bytes);
}

#[test]
fn global_palette_matches_csharp() {
    let frames = frames();
    let bytes = encode(|gif| {
        gif.quantizer_type = QuantizerType::Neural;
        gif.sampling_factor = 5;
        gif.maximum_number_color = 256;
        gif.use_global_palette = true;
        gif.use_dithering = false;
        gif.set_canvas_size(W as i32, H as i32);
        gif.build_global_palette(&frames);
        for f in &frames {
            gif.add_frame(f, Rect { x: 0, y: 0, width: W as i32, height: H as i32 }, 100, false);
        }
    });
    compare("golden_global.gif", &bytes);
}

#[test]
fn transparency_matches_csharp() {
    let frames = frames();
    let bytes = encode(|gif| {
        gif.quantizer_type = QuantizerType::Octree;
        gif.maximum_number_color = 128;
        gif.use_global_palette = false;
        gif.use_dithering = false;
        gif.set_canvas_size(W as i32, H as i32);

        let mut punched = frames[0].clone();
        for i in (3..punched.len()).step_by(4) {
            punched[i] = 0; // all transparent
        }
        gif.add_frame(&punched, Rect { x: 0, y: 0, width: W as i32, height: H as i32 }, 100, true);
        gif.add_frame(&frames[1], Rect { x: 0, y: 0, width: W as i32, height: H as i32 }, 100, true);
    });
    compare("golden_transparent.gif", &bytes);
}

/// Two deterministic BGRA frames; the second differs in a 2x2 square at (4,3).
fn frames() -> Vec<Vec<u8>> {
    let first = gradient();
    let mut second = first.clone();
    for dy in 0..2usize {
        for dx in 0..2usize {
            let i = ((3 + dy) * W + 4 + dx) * 4;
            second[i] = 0;
            second[i + 1] = 0;
            second[i + 2] = 255;
            second[i + 3] = 255;
        }
    }
    vec![first, second]
}

fn gradient() -> Vec<u8> {
    let mut pixels = vec![0u8; W * H * 4];
    for y in 0..H {
        for x in 0..W {
            let i = (y * W + x) * 4;
            pixels[i] = (x * 255 / (W - 1).max(1)) as u8;
            pixels[i + 1] = (y * 255 / (H - 1).max(1)) as u8;
            pixels[i + 2] = ((x + y) * 127 / (W + H - 2).max(1)) as u8;
            pixels[i + 3] = 255;
        }
    }
    pixels
}
