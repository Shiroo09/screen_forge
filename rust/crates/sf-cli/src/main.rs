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
        Some("record") => {
            // record <out.gif> <seconds> [fps] — real-time capture to a GIF,
            // dropping frames identical to the last kept one.
            let out = args.get(1).ok_or("record: missing output path")?;
            let seconds: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(3.0);
            let fps: i32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(10);
            let stats = record_gif(out, seconds, fps)?;
            Ok(format!(
                "wrote {out} ({} frames kept, {} identical dropped, {:.1}s)",
                stats.kept, stats.dropped, stats.elapsed_s
            ))
        }
        Some("settings") => {
            // settings [path] — load settings and print the normalized result.
            let path = args
                .get(1)
                .map(std::path::PathBuf::from)
                .unwrap_or_else(sf_settings::settings_path);
            let (settings, first_run) = sf_settings::AppSettings::load(&path);
            Ok(format!(
                "settings {} (first_run={first_run}): format={:?} gif_fps={} quality={} region_hotkey={:?}",
                path.display(),
                settings.output_format,
                settings.gif.fps,
                settings.quality,
                settings.region_hotkey.display(),
            ))
        }
        _ => Err("usage: screenforge <png|gif|crop|record|settings> <out> ...".to_string()),
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

/// Result of a real-time recording.
pub struct RecordStats {
    pub kept: usize,
    pub dropped: usize,
    pub elapsed_s: f64,
}

/// Real-time screen recording to an animated GIF.
///
/// Matches the C# recorder's split: capture is fast (frames are Deflate-compressed
/// as they arrive, never held raw), and palette/quantization happen once at export
/// using a **global palette** — so per-frame encoding is a cached color map, not a
/// full NeuQuant pass per frame.
fn record_gif(path: &str, seconds: f64, fps: i32) -> Result<RecordStats, String> {
    let fps = fps.clamp(1, 30);
    let total_frames = ((seconds.max(0.1)) * fps as f64).round() as usize;
    let delay_ms = (1000.0 / fps as f64).round() as i32;

    let start = std::time::Instant::now();
    let mut kept = 0usize;
    let mut dropped = 0usize;
    let mut last_kept: Option<Vec<u8>> = None;
    let mut interval = std::time::Duration::from_millis((1000 / fps) as u64);

    let first = sf_capture::capture_virtual_screen()?;
    let (w, h) = (first.width, first.height);

    // Capture phase: compress each frame as it arrives. Memory stays bounded to
    // the compressed size, not frames × 29 MB raw.
    let mut store = sf_gif::frame_store::FrameStore::new((w * h * 4) as usize);
    let mut samples: Vec<Vec<u8>> = Vec::new();
    let mut first = Some(first);

    for i in 0..total_frames {
        let frame_start = std::time::Instant::now();

        let shot = match first.take() {
            Some(f) => f,
            None => sf_capture::capture_virtual_screen()?,
        };

        let is_same = last_kept
            .as_ref()
            .is_some_and(|prev| sf_edit::similarity_of(prev, &shot.bgra) >= 100.0);

        if is_same {
            dropped += 1;
        } else {
            // Keep a few raw frames as palette samples (thinned, capped in the encoder).
            if samples.len() < 12 {
                samples.push(shot.bgra.clone());
            }
            store.add(&shot.bgra);
            last_kept = Some(shot.bgra);
            kept += 1;
        }

        if i + 1 < total_frames {
            let spent = frame_start.elapsed();
            if spent < interval {
                std::thread::sleep(interval - spent);
            } else {
                interval = interval.saturating_mul(2);
            }
        }
    }

    // Export phase: one global palette from the samples, then stream each frame.
    let file = std::fs::File::create(path).map_err(|e| format!("create {path}: {e}"))?;
    let writer = std::io::BufWriter::new(file);
    let mut gif = GifFile::new(writer);
    gif.set_canvas_size(w, h);
    gif.maximum_number_color = 256;
    gif.use_global_palette = true;
    gif.quantizer_type = sf_gif::QuantizerType::Octree; // fast; Neural is available for quality
    gif.build_global_palette(&samples);

    let packed = store.drain_packed();
    for block in packed {
        // Decode one frame at a time; it is released before the next.
        let mut bgra = vec![0u8; (w * h * 4) as usize];
        sf_gif::frame_store::decompress_into(&block, &mut bgra);
        gif.add_frame(&bgra, Rect { x: 0, y: 0, width: w, height: h }, delay_ms, false);
    }

    gif.finish();

    Ok(RecordStats {
        kept,
        dropped,
        elapsed_s: start.elapsed().as_secs_f64(),
    })
}
