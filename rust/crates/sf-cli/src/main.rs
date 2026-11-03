//! Minimal ScreenForge CLI, proving the Rust pipeline end to end: capture the
//! screen, then write it as PNG or as an animated GIF using the ported encoder.
//!
//! Usage:
//!   screenforge png <out.png>                       # one screenshot
//!   screenforge gif <out.gif> <frames> <delay_ms>   # an animated GIF
//!   screenforge crop <out.png> <x> <y> <w> <h>      # a screen region

use std::process::ExitCode;

use sf_gif::gif_file::{GifFile, Rect};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(msg) => {
            println!("{msg}");
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("screenforge: {msg}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<String, String> {
    match args.first().map(String::as_str) {
        Some("png") => {
            let out = args.get(1).ok_or("png: missing output path")?;
            let shot = sf_capture::capture_virtual_screen()?;
            write_png(out, shot.width, shot.height, &shot.bgra)?;
            Ok(format!("wrote {out} ({}x{})", shot.width, shot.height))
        }
        Some("crop") => {
            let out = args.get(1).ok_or("crop: missing output path")?;
            let x: i32 = args.get(2).ok_or("crop: missing x")?.parse().map_err(|_| "bad x")?;
            let y: i32 = args.get(3).ok_or("crop: missing y")?.parse().map_err(|_| "bad y")?;
            let w: i32 = args.get(4).ok_or("crop: missing w")?.parse().map_err(|_| "bad w")?;
            let h: i32 = args.get(5).ok_or("crop: missing h")?.parse().map_err(|_| "bad h")?;

            let shot = sf_capture::capture_region(x, y, w, h)?;
            write_png(out, shot.width, shot.height, &shot.bgra)?;
            Ok(format!("wrote {out} ({}x{})", shot.width, shot.height))
        }
        Some("gif") => {
            let out = args.get(1).ok_or("gif: missing output path")?;
            let frames: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10);
            let delay_ms: i32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(100);
            write_gif(out, frames, delay_ms)?;
            Ok(format!("wrote {out} ({frames} frames, {delay_ms} ms)"))
        }
        _ => Err("usage: screenforge <png|gif|crop> <out> ...".to_string()),
    }
}

/// Encodes BGRA to PNG with the pure-Rust `png` crate (no GDI+ dependency).
fn write_png(path: &str, width: i32, height: i32, bgra: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("create {path}: {e}"))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);

    let mut writer = encoder.write_header().map_err(|e| format!("png header: {e}"))?;

    // The pipeline is BGRA; PNG wants RGBA. Swap the R and B bytes.
    let mut rgba = Vec::with_capacity(bgra.len());
    for px in bgra.chunks_exact(4) {
        rgba.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
    }

    writer.write_image_data(&rgba).map_err(|e| format!("png data: {e}"))?;
    Ok(())
}

/// Captures `frames` screenshots spaced by `delay_ms` and encodes them as a GIF.
fn write_gif(path: &str, frames: usize, delay_ms: i32) -> Result<(), String> {
    let frames = frames.clamp(1, 600);

    let first = sf_capture::capture_virtual_screen()?;
    let (w, h) = (first.width, first.height);

    let file = std::fs::File::create(path).map_err(|e| format!("create {path}: {e}"))?;
    // Buffer writes so the GIF streams to disk instead of accumulating.
    let mut writer = std::io::BufWriter::new(file);
    let mut gif = GifFile::new(&mut writer);
    gif.set_canvas_size(w, h);
    gif.maximum_number_color = 256;

    gif.add_frame(&first.bgra, Rect { x: 0, y: 0, width: w, height: h }, delay_ms, false);

    for i in 1..frames {
        std::thread::sleep(std::time::Duration::from_millis(delay_ms.max(1) as u64));
        let shot = sf_capture::capture_region(0, 0, w, h)?;
        gif.add_frame(&shot.bgra, Rect { x: 0, y: 0, width: w, height: h }, delay_ms, false);
        eprintln!("  frame {}/{frames}", i + 1);
    }

    gif.finish();
    Ok(())
}
