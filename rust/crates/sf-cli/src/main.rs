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

fn parse_u32(s: &str) -> Option<u32> {
    let s = s.trim();
    match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(hex) => u32::from_str_radix(hex, 16).ok(),
        None => s.parse().ok(),
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
        Some("shot") => {
            // shot [out.png] — capture and put it on the clipboard, then save.
            let out = args.get(1).cloned().unwrap_or_else(|| {
                format!("{}/sf-shot.png", std::env::temp_dir().display())
            });
            let shot = sf_capture::capture_virtual_screen()?;
            sf_capture::copy_bgra_to_clipboard(shot.width, shot.height, &shot.bgra)?;
            write_png(&out, shot.width, shot.height, &shot.bgra)?;
            Ok(format!("copied to clipboard + wrote {out} ({}x{})", shot.width, shot.height))
        }
        Some("edit") => {
            // edit <out.png> — pick a region, annotate it, save the result.
            let out = args.get(1).ok_or("edit: missing output path")?;
            edit_flow(out)
        }
        Some("annotate") => {
            // annotate <out.png> — capture the screen and draw a sample
            // annotation, proving capture -> rasterizer -> save end to end.
            let out = args.get(1).ok_or("annotate: missing output path")?;
            let mut shot = sf_capture::capture_virtual_screen()?;
            annotate_sample(&mut shot);
            write_png(out, shot.width, shot.height, &shot.bgra)?;
            Ok(format!("wrote annotated {out} ({}x{})", shot.width, shot.height))
        }
        Some("mp4") => {
            // mp4 <out.mp4> <seconds> [fps] — real-time screen recording to H.264.
            let out = args.get(1).ok_or("mp4: missing output path")?;
            let seconds: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(3.0);
            let fps: i32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(15);
            let stats = record_mp4(out, seconds, fps)?;
            Ok(format!(
                "wrote {out} ({} frames, {:.1}s)",
                stats, seconds.max(0.1)
            ))
        }
        Some("select") => {
            // select <out.png> — show the region picker, then save the selection.
            let out = args.get(1).ok_or("select: missing output path")?;
            let rect = sf_capture::overlay_window::pick_region()
                .ok_or("selection cancelled or too small")?;
            let (x, y, w, h) = rect;
            let shot = sf_capture::capture_region(x, y, w, h)?;
            write_png(out, shot.width, shot.height, &shot.bgra)?;
            Ok(format!("selected ({x},{y},{w},{h}) -> wrote {out}"))
        }
        Some("run") => {
            // run [seconds] — register the capture hotkey (Alt+Shift+S), stay in
            // the tray, and save a PNG each time it fires. Proves the interactive
            // path end to end: global hotkey -> capture -> file + notification.
            let seconds: f64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            // Optional hex modifiers + virtual-key, e.g. "run 0 6 0x78" (Ctrl+Shift+F9).
            let mods: u32 = args.get(2).and_then(|s| parse_u32(s)).unwrap_or(0x1 | 0x4);
            let vk: u32 = args.get(3).and_then(|s| parse_u32(s)).unwrap_or(0x53);
            run_tray(seconds, mods, vk)
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
        _ => Err("usage: screenforge <png|gif|crop|record|mp4|shot|select|edit|annotate|run|settings> ...".to_string()),
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

/// Interactive shell: registers the capture hotkey and saves a PNG on each press.
///
/// `seconds` 0 means run until the process is killed; a positive value exits after
/// that many seconds, which makes it usable from a test.
#[cfg(windows)]
fn run_tray(seconds: f64, modifiers: u32, vk: u32) -> Result<String, String> {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    let count = Arc::new(AtomicU32::new(0));
    let out_dir = std::env::temp_dir().join("sf-out");
    std::fs::create_dir_all(&out_dir).ok();

    let count_cb = Arc::clone(&count);
    let dir_cb = out_dir.clone();

    let reg = sf_capture::hotkey::register(modifiers, vk, move || {
        let n = count_cb.fetch_add(1, Ordering::SeqCst) + 1;
        // Lightshot flow: let the user drag a region, crop it, put it on the
        // clipboard and save it. Cancel (Esc/right-click) falls back to nothing.
        let edit = match sf_capture::editor_window::edit_region() {
            Some(e) => e,
            None => {
                eprintln!("hotkey {n}: cancelled");
                return;
            }
        };
        let (x, y, w, h) = edit.region;
        let result = sf_capture::capture_region(x, y, w, h).map(|mut shot| {
            // Shapes are in virtual-screen coords; shift into the crop.
            use sf_draw::canvas::Canvas;
            let mut canvas = Canvas::new(&mut shot.bgra, shot.width, shot.height);
            for shape in &edit.shapes {
                sf_draw::model::render(&mut canvas, &shape.translate(-(x as f32), -(y as f32)));
            }
            shot
        });

        match result {
            Ok(shot) => {
                let path = dir_cb.join(format!("hotkey-{n}.png"));
                if let Err(e) = sf_capture::copy_bgra_to_clipboard(shot.width, shot.height, &shot.bgra) {
                    eprintln!("hotkey {n}: clipboard failed: {e}");
                }
                match write_png(path.to_str().unwrap_or("out.png"), shot.width, shot.height, &shot.bgra) {
                    // Log, don't pop a modal: the callback runs on the hotkey
                    // thread and a blocking dialog would stall further hotkeys.
                    Ok(()) => eprintln!("hotkey {n}: saved {} ({}x{})", path.display(), shot.width, shot.height),
                    Err(e) => eprintln!("hotkey {n}: png failed: {e}"),
                }
            }
            Err(e) => eprintln!("hotkey {n}: capture failed: {e}"),
        }
    })?;

    // System tray icon (left-click triggers the same capture as the hotkey).
    let tray = {
        let dir = out_dir.clone();
        let count = Arc::clone(&count);
        sf_capture::tray::install("ScreenForge — ekran görüntüsü", move || {
            let n = count.fetch_add(1, Ordering::SeqCst) + 1;
            if let Some((x, y, w, h)) = sf_capture::overlay_window::pick_region() {
                if let Ok(shot) = sf_capture::capture_region(x, y, w, h) {
                    let path = dir.join(format!("tray-{n}.png"));
                    let _ = write_png(path.to_str().unwrap_or("out.png"), shot.width, shot.height, &shot.bgra);
                    eprintln!("tray {n}: saved {}", path.display());
                }
            }
        })
        .ok()
    };

    eprintln!("ScreenForge ready — press the hotkey, drag a region to capture");

    if seconds > 0.0 {
        std::thread::sleep(std::time::Duration::from_secs_f64(seconds));
        let n = count.load(Ordering::SeqCst);
        reg.stop();
        if let Some(t) = tray {
            t.close();
        }
        Ok(format!("ran {seconds}s, {n} capture(s) triggered"))
    } else {
        // Stay alive; the hotkey thread owns the message loop.
        loop {
            std::thread::sleep(std::time::Duration::from_secs(3600));
        }
    }
}

#[cfg(not(windows))]
fn run_tray(_seconds: f64, _modifiers: u32, _vk: u32) -> Result<String, String> {
    Err("the tray shell is only supported on Windows".to_string())
}

/// Draws a sample annotation (rect, arrow, ellipse, numbered step, pen) onto a
/// captured frame. The interactive editor will drive the same `sf_draw` calls
/// from mouse input; this shows the rasterizer works on real captured pixels.
fn annotate_sample(shot: &mut sf_capture::Captured) {
    use sf_draw::canvas::{Canvas, Rgba};
    use sf_draw::shapes::{self, Stroke};

    let orange = Rgba::rgb(0xEA6F12);
    let red = Rgba::rgb(0xE5484D);
    let white = Rgba::rgb(0xFFFFFF);

    let mut c = Canvas::new(&mut shot.bgra, shot.width, shot.height);

    // A rectangle near the top-left, a bend arrow, an ellipse and a step badge —
    // all scaled to the screen so it is visible on any resolution.
    let s = (shot.width.min(shot.height) as f32 / 60.0).max(2.0);
    let (ox, oy) = (120.0 * s / 4.0 * 4.0, 120.0 * s / 4.0 * 4.0);

    shapes::draw_rect(&mut c, ox, oy, 40.0 * s, 24.0 * s, Stroke::new(orange, 2.0 * s));
    shapes::draw_arrow(&mut c, ox + 50.0 * s, oy + 30.0 * s, ox + 120.0 * s, oy + 10.0 * s, Stroke::new(red, 2.0 * s));
    shapes::draw_ellipse(&mut c, ox + 150.0 * s, oy, 36.0 * s, 36.0 * s, Stroke::new(white, 2.0 * s));
    shapes::draw_step(&mut c, ox + 20.0 * s, oy + 60.0 * s, 14.0 * s, 1, red, white);
    shapes::draw_stroke_path(
        &mut c,
        &[
            (ox + 60.0 * s, oy + 70.0 * s),
            (ox + 80.0 * s, oy + 90.0 * s),
            (ox + 110.0 * s, oy + 60.0 * s),
        ],
        Stroke::new(orange, 2.0 * s),
    );
}

/// Interactive flow: pick a region, annotate it, crop to the region and save.
fn edit_flow(out: &str) -> Result<String, String> {
    let result = sf_capture::editor_window::edit_region()
        .ok_or("edit cancelled")?;

    let (x, y, w, h) = result.region;
    let mut shot = sf_capture::capture_region(x, y, w, h)?;

    // Shapes were collected in virtual-screen coords; shift them into the crop.
    let (ox, oy) = (x as f32, y as f32);
    {
        use sf_draw::canvas::Canvas;
        let mut canvas = Canvas::new(&mut shot.bgra, shot.width, shot.height);
        for shape in &result.shapes {
            sf_draw::model::render(&mut canvas, &shape.translate(-ox, -oy));
        }
    }

    write_png(out, shot.width, shot.height, &shot.bgra)?;
    Ok(format!(
        "edited ({x},{y},{w},{h}) with {} shape(s) -> wrote {out}",
        result.shapes.len()
    ))
}

/// Real-time screen recording to an H.264 MP4 via Media Foundation.
fn record_mp4(path: &str, seconds: f64, fps: i32) -> Result<usize, String> {
    use sf_media::bits_per_second;

    let fps = fps.clamp(1, 60);
    let total = ((seconds.max(0.1)) * fps as f64).round() as usize;

    // H.264 wants even dimensions; cap the long edge (the C# app uses 2560) so a
    // 4K desktop does not encode at full size and crawl.
    let first = sf_capture::capture_virtual_screen()?;
    let (w, h) = sf_media::cap_long_edge(first.width, first.height, sf_media::MAX_LONG_EDGE);
    if w == 0 || h == 0 {
        return Err("screen capture is degenerate".to_string());
    }

    let bitrate = bits_per_second(sf_settings::VideoQuality::High, w, h);
    let mut writer = sf_record::Mp4Writer::new(path, w, h, fps, bitrate)?;
    let frame_dur = writer.frame_duration();

    let interval = std::time::Duration::from_millis((1000 / fps) as u64);
    let mut first = Some(first);

    for i in 0..total {
        let start = std::time::Instant::now();
        let shot = match first.take() {
            Some(f) => f,
            None => sf_capture::capture_virtual_screen()?,
        };

        // Crop/resize to the even, capped frame if the screen was reduced.
        let frame = if shot.width == w && shot.height == h {
            shot.bgra
        } else {
            sf_edit::image_ops::resize_pixels(&shot.bgra, shot.width, shot.height, w, h)
        };

        writer.write_bgra(&frame, i as i64 * frame_dur)?;

        if i + 1 < total {
            let spent = start.elapsed();
            if spent < interval {
                std::thread::sleep(interval - spent);
            }
        }
    }

    writer.finish()?;
    Ok(total)
}
