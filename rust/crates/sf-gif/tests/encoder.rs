//! Mirrors `ScreenForge.Tests/GifEncoderTests.cs` so the Rust port is held to the
//! same behavioural contract as the C# original.

use sf_gif::frame_store::FrameStore;
use sf_gif::gif_file::{size_field, GifFile, Rect};
use sf_gif::palette::PaletteMap;
use sf_gif::quantize::{NeuralQuantizer, OctreeQuantizer, Quantizer};
use sf_gif::Rgb;

#[test]
fn size_field_picks_smallest_power_of_two_that_fits() {
    assert_eq!(size_field(2), 0);
    assert_eq!(size_field(4), 1);
    assert_eq!(size_field(5), 2);
    assert_eq!(size_field(64), 5);
    assert_eq!(size_field(256), 7);
}

#[test]
fn palette_map_finds_nearest_color() {
    let palette = [
        Rgb::new(0, 0, 0),
        Rgb::new(255, 0, 0),
        Rgb::new(0, 255, 0),
    ];
    let mut map = PaletteMap::new(&palette, -1);

    assert_eq!(map.map(10, 10, 10), 0);
    assert_eq!(map.map(240, 12, 8), 1);
    assert_eq!(map.map(8, 230, 12), 2);
}

#[test]
fn palette_map_never_returns_transparent_slot() {
    let palette = [
        Rgb::new(255, 255, 255),
        Rgb::new(0, 0, 0), // transparent slot
    ];
    let mut map = PaletteMap::new(&palette, 1);

    // An exact black match would hit the transparent slot; an opaque pixel must not.
    assert_eq!(map.map(0, 0, 0), 0);
    assert_eq!(map.transparent_index(), 1);
}

#[test]
fn octree_quantizer_produces_palette_within_limit() {
    let pixels = make_gradient(32, 32);
    let mut q = OctreeQuantizer::new(16);

    let palette = q.quantize(&pixels);

    assert!(!palette.is_empty());
    assert!(palette.len() <= 16, "palette returned {} colors, limit 16", palette.len());
}

#[test]
fn neural_quantizer_produces_requested_palette_size() {
    let pixels = make_gradient(32, 32);
    let mut q = NeuralQuantizer::new(1, 32);

    let palette = q.quantize(&pixels);

    assert_eq!(palette.len(), 32);
}

#[test]
fn frame_store_roundtrips_frames() {
    let mut store = FrameStore::new(4 * 4 * 4);
    let frame = make_gradient(4, 4);

    store.add(&frame);
    store.add(&frame);
    assert_eq!(store.count(), 2);
    assert!(store.ratio() > 1.0, "gradient should compress");

    let restored = store.get(1);
    assert_eq!(restored, frame);
}

#[test]
fn gyif_writes_valid_header_and_trailer() {
    let frames = make_mostly_static_frames(8, 8, 3);
    let bytes = write_gif(&frames, 8, 8);

    assert!(bytes.len() > 16);
    assert_eq!(&bytes[0..6], b"GIF89a");
    assert_eq!(*bytes.last().unwrap(), 0x3b); // trailer
}

#[test]
fn gif_logical_screen_matches_requested_size() {
    let frames = make_mostly_static_frames(8, 8, 3);
    let bytes = write_gif(&frames, 8, 8);
    let width = bytes[6] as i32 | (bytes[7] as i32) << 8;
    let height = bytes[8] as i32 | (bytes[9] as i32) << 8;

    assert_eq!(width, 8);
    assert_eq!(height, 8);
}

#[test]
fn global_palette_emits_single_palette() {
    let frames = make_mostly_static_frames(64, 64, 10);

    let global = write_gif_opts(&frames, 64, 64, true, false);
    let local = write_gif_opts(&frames, 64, 64, false, false);

    // Global palette flag: bit 7 of the logical screen descriptor packed field.
    assert_ne!(global[10] & 0x80, 0);
    assert!(global.len() < local.len(), "global {} vs local {}", global.len(), local.len());
}

#[test]
fn dithering_produces_readable_gif() {
    let frames = [make_gradient(48, 48)];
    let mut out = Vec::new();
    {
        let mut gif = GifFile::new(&mut out);
        gif.use_dithering = true;
        gif.maximum_number_color = 32;
        gif.set_canvas_size(48, 48);
        gif.add_frame(&frames[0], Rect { x: 0, y: 0, width: 48, height: 48 }, 100, false);
        gif.finish();
    }

    assert_eq!(&out[0..6], b"GIF89a");
    assert_eq!(*out.last().unwrap(), 0x3b);
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn write_gif(frames: &[Vec<u8>], width: i32, height: i32) -> Vec<u8> {
    write_gif_opts(frames, width, height, false, false)
}

fn write_gif_opts(
    frames: &[Vec<u8>],
    width: i32,
    height: i32,
    global_palette: bool,
    dithering: bool,
) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut gif = GifFile::new(&mut out);
        gif.use_global_palette = global_palette;
        gif.use_dithering = dithering;
        gif.set_canvas_size(width, height);

        if global_palette {
            gif.build_global_palette(frames);
        }

        for frame in frames {
            gif.add_frame(frame, Rect { x: 0, y: 0, width, height }, 100, false);
        }
        gif.finish();
    }
    out
}

fn make_gradient(width: usize, height: usize) -> Vec<u8> {
    let mut pixels = vec![0u8; width * height * 4];
    for y in 0..height {
        for x in 0..width {
            let i = (y * width + x) * 4;
            pixels[i] = (x * 255 / (width.saturating_sub(1)).max(1)) as u8;
            pixels[i + 1] = (y * 255 / (height.saturating_sub(1)).max(1)) as u8;
            pixels[i + 2] = ((x + y) * 127 / (width + height).saturating_sub(2).max(1)) as u8;
            pixels[i + 3] = 255;
        }
    }
    pixels
}

/// Frames where only a small square changes — the typical delta-optimization case.
fn make_mostly_static_frames(width: usize, height: usize, count: usize) -> Vec<Vec<u8>> {
    let base = make_gradient(width, height);
    let mut frames = Vec::with_capacity(count);

    for f in 0..count {
        let mut frame = base.clone();
        let px = f % (width.saturating_sub(2)).max(1);
        let py = f % (height.saturating_sub(2)).max(1);

        for dy in 0..2 {
            for dx in 0..2 {
                let i = ((py + dy) * width + px + dx) * 4;
                frame[i] = 0;
                frame[i + 1] = 0;
                frame[i + 2] = 255;
                frame[i + 3] = 255;
            }
        }

        frames.push(frame);
    }

    frames
}
