//! Minimal ScreenForge CLI, proving the Rust pipeline end to end: capture the
//! screen, then write it as PNG or as an animated GIF using the ported encoder.
//!
//! Usage:
//!   screenforge png <out.png>                       # one screenshot
//!   screenforge gif <out.gif> <frames> <delay_ms>   # an animated GIF
//!   screenforge crop <out.png> <x> <y> <w> <h>      # a screen region

mod gifedit;

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

/// Reads `--name value` from the argument list.
fn flag_value(args: &[String], name: &str) -> Option<i32> {
    let idx = args.iter().position(|a| a == name)?;
    args.get(idx + 1).and_then(|v| v.parse().ok())
}

/// Parses "r,g,b" (0-255 each) into a triple.
fn parse_rgb(s: Option<&str>) -> Option<[u8; 3]> {
    let s = s?;
    let parts: Vec<u8> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
    if parts.len() == 3 {
        Some([parts[0], parts[1], parts[2]])
    } else {
        None
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
            write_by_extension(out, shot.width, shot.height, &shot.bgra)?;
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
            write_by_extension(&out, shot.width, shot.height, &shot.bgra)?;
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
            write_by_extension(out, shot.width, shot.height, &shot.bgra)?;
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
            write_by_extension(out, shot.width, shot.height, &shot.bgra)?;
            Ok(format!("selected ({x},{y},{w},{h}) -> wrote {out}"))
        }
        Some("translate") => {
            // translate <text...> [--to tr] — translate text (default: settings native).
            let joined = args[1..].join(" ");
            let (settings, _) = sf_settings::AppSettings::load(&sf_settings::settings_path());
            let target = if settings.translate_native_language.trim().is_empty() {
                "tr".to_string()
            } else {
                settings.translate_native_language.clone()
            };
            let result = sf_net::translate::translate(&joined, &target)?;
            Ok(format!("[{}] {}", target, result.text))
        }
        Some("reverse") => {
            // reverse <image> [yandex|lens] — print a reverse-search URL.
            let path = args.get(1).ok_or("reverse: missing image path")?;
            let engine = match args.get(2).map(String::as_str) {
                Some("lens") => sf_net::reverse_http::Engine::GoogleLens,
                _ => sf_net::reverse_http::Engine::Yandex,
            };
            let bytes = std::fs::read(path).map_err(|e| format!("read {path}: {e}"))?;
            let url = sf_net::reverse_http::search_url(&bytes, engine)?;
            Ok(format!("reverse search URL: {url}"))
        }
        Some("gifedit") => {
            // gifedit <in.gif> <out.gif> [--keep N] [--remove N] [--reverse]
            let input = args.get(1).ok_or("gifedit: missing input gif")?;
            let output = args.get(2).ok_or("gifedit: missing output gif")?;
            let keep = flag_value(args, "--keep").unwrap_or(2).max(1) as usize;
            let remove = flag_value(args, "--remove").unwrap_or(1).max(1) as usize;
            let reverse = args.iter().any(|a| a == "--reverse");

            let decoded = gifedit::decode(input)?;
            let before = decoded.frames.len();
            let after = gifedit::encode(
                output,
                decoded.width,
                decoded.height,
                &decoded.frames,
                keep,
                remove,
                reverse,
            )?;
            Ok(format!(
                "gifedit: {before} -> {after} frames, wrote {output} ({}x{})",
                decoded.width, decoded.height
            ))
        }
        Some("removebg") => {
            // removebg <in.png> <out.png> <model.onnx> [r,g,b] — cut the subject
            // out and place it on a solid background.
            let input = args.get(1).ok_or("removebg: missing input image")?;
            let output = args.get(2).ok_or("removebg: missing output image")?;
            let model_path = args.get(3).ok_or("removebg: missing u2netp.onnx path")?;

            let img = image::open(input).map_err(|e| format!("open {input}: {e}"))?;
            let rgba = img.to_rgba8();
            let (w, h) = (rgba.width() as i32, rgba.height() as i32);
            // RGBA → BGRA for the model input.
            let mut bgra = rgba.into_raw();
            for px in bgra.chunks_exact_mut(4) {
                px.swap(0, 2);
            }

            let background = parse_rgb(args.get(4).map(String::as_str)).unwrap_or([255, 255, 255]);

            let mut model = sf_bg::model::SubjectModel::load(model_path)?;
            let (mask, _side) = model.segment(&bgra, w, h)?;
            let composed = sf_bg::composite_on_solid(&bgra, &mask, background);

            write_by_extension(output, w, h, &composed)?;
            Ok(format!("removebg: {w}x{h} konu kesildi -> {output}"))
        }
        Some("subtitle") => {
            // subtitle <text> — show a live-subtitle box over the screen (Esc çıkış).
            let text = args[1..].join(" ");
            let (settings, _) = sf_settings::AppSettings::load(&sf_settings::settings_path());
            let (vx, vy, vw, vh) = sf_capture::virtual_screen_bounds();
            let _ = (vx, vy);
            let screen = (0, 0, vw, vh);
            let style = sf_capture::subtitle_window::SubtitleStyle {
                back: sf_capture::subtitle_window::SubtitleStyle::parse_color(&settings.subtitle.back_color, [0x10, 0x12, 0x18, 0xFF]),
                text: sf_capture::subtitle_window::SubtitleStyle::parse_color(&settings.subtitle.text_color, [0xFF, 0xFF, 0xFF, 0xFF]),
                border: sf_capture::subtitle_window::SubtitleStyle::parse_color(&settings.subtitle.border_color, [0xFF, 0xFF, 0xFF, 0xFF]),
                opacity: settings.subtitle.opacity as f32,
                border_thickness: settings.subtitle.border_thickness as f32,
            };
            // Use the configured box if set, else a centered default.
            let rect = if settings.subtitle.has_box() {
                (settings.subtitle.box_x, settings.subtitle.box_y, settings.subtitle.box_w, settings.subtitle.box_h)
            } else {
                (screen.0 + vw / 6, screen.1 + vh * 4 / 5, vw * 2 / 3, 70)
            };
            sf_capture::subtitle_window::run(rect, style, text);
            Ok("altyazı kapandı".to_string())
        }
        Some("presenter") => {
            // presenter — live drawing overlay (Esc to exit; 1=pen 2=laser 3=spotlight).
            eprintln!("sunum katmanı açılıyor — Esc ile çık, 1=kalem 2=lazer 3=spotlight");
            sf_capture::presenter_window::run();
            Ok("sunum katmanı kapandı".to_string())
        }
        Some("update") => {
            // update <manifest-url> [current-version] [--apply] — check for and
            // optionally download a newer release. No manifest URL configured →
            // reports that updates are not set up.
            let url = args.get(1).ok_or("update: missing manifest URL")?;
            let current = args.get(2).cloned().unwrap_or_else(|| "0.1.0".to_string());
            match sf_net::update::check(url, &current)? {
                Some(release) => {
                    if args.iter().any(|a| a == "--download") {
                        let path = sf_net::update::download_to_new(&release.url)?;
                        Ok(format!("yeni sürüm {} indirildi: {}", release.version, path.display()))
                    } else {
                        Ok(format!("yeni sürüm mevcut: {} ({})", release.version, release.url))
                    }
                }
                None => Ok(format!("güncel (sürüm {current})")),
            }
        }
        Some("gui") => {
            // gui — open the settings window, then save if accepted.
            let (mut settings, _) = sf_settings::AppSettings::load(&sf_settings::settings_path());
            if sf_capture::settings_window::show(&mut settings) {
                settings
                    .save(&sf_settings::settings_path())
                    .map_err(|e| format!("ayarlar kaydedilemedi: {e}"))?;
                // Apply the startup preference to the registry.
                let _ = sf_settings::set_startup_enabled(settings.launch_at_startup);
                Ok(format!(
                    "ayarlar kaydedildi: {} (başlangıç={})",
                    sf_settings::settings_path().display(),
                    settings.launch_at_startup
                ))
            } else {
                Ok("ayarlar değiştirilmedi (iptal)".to_string())
            }
        }
        Some("run") => {
            // run [seconds] — register the capture hotkey (Alt+Shift+S), stay in
            // the tray, and save a PNG each time it fires. Proves the interactive
            // path end to end: global hotkey -> capture -> file + notification.
            let seconds: f64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.0);

            // Hotkey and output come from settings; CLI overrides only for tests.
            let (settings, first_run) = sf_settings::AppSettings::load(&sf_settings::settings_path());
            if first_run {
                eprintln!("(yeni ayar dosyası oluşturuldu: {})", sf_settings::settings_path().display());
            }

            let mods: u32 = match args.get(2) {
                Some(s) => parse_u32(s).ok_or("bad modifier flags")?,
                None => modifiers_from(&settings.region_hotkey),
            };
            let vk: u32 = match args.get(3) {
                Some(s) => parse_u32(s).ok_or("bad virtual key")?,
                None => vk_from(&settings.region_hotkey).unwrap_or(0x53),
            };
            let out_dir = std::path::PathBuf::from(&settings.save_directory);
            eprintln!(
                "ayarlar: format={:?} kalite={} klasör={} kısayol={}",
                settings.output_format, settings.quality, out_dir.display(), settings.region_hotkey.display()
            );
            run_tray(seconds, mods, vk, out_dir, settings.output_format, settings.quality)
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
        _ => Err("usage: screenforge <png|gif|crop|record|mp4|shot|select|edit|gifedit|annotate|reverse|translate|removebg|update|presenter|subtitle|gui|run|settings> ...".to_string()),
    }
}

/// Chooses the output format from the file extension (default PNG).
fn write_by_extension(path: &str, width: i32, height: i32, bgra: &[u8]) -> Result<(), String> {
    let lower = path.to_lowercase();
    let format = if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        sf_settings::ImageFormat::Jpeg
    } else if lower.ends_with(".webp") {
        sf_settings::ImageFormat::Webp
    } else {
        sf_settings::ImageFormat::Png
    };
    write_image(path, width, height, bgra, format, 92)
}

/// Encodes BGRA to PNG with the pure-Rust `png` crate (no GDI+ dependency).
fn write_png(path: &str, width: i32, height: i32, bgra: &[u8]) -> Result<(), String> {
    write_image(path, width, height, bgra, sf_settings::ImageFormat::Png, 92)
}

/// Encodes BGRA to the requested format. `quality` (1-100) applies to JPEG/WebP.
fn write_image(
    path: &str,
    width: i32,
    height: i32,
    bgra: &[u8],
    format: sf_settings::ImageFormat,
    quality: i32,
) -> Result<(), String> {
    if width <= 0 || height <= 0 || bgra.len() < (width * height * 4) as usize {
        return Err("invalid image buffer".to_string());
    }

    // The pipeline is BGRA; encoders want RGBA. Swap R and B once.
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for px in bgra.chunks_exact(4) {
        rgba.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
    }

    match format {
        sf_settings::ImageFormat::Png => {
            let file = std::fs::File::create(path).map_err(|e| format!("create {path}: {e}"))?;
            let mut encoder =
                png::Encoder::new(std::io::BufWriter::new(file), width as u32, height as u32);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().map_err(|e| format!("png header: {e}"))?;
            writer.write_image_data(&rgba).map_err(|e| format!("png data: {e}"))?;
            Ok(())
        }
        sf_settings::ImageFormat::Jpeg | sf_settings::ImageFormat::Webp => {
            let img: image::RgbaImage =
                image::ImageBuffer::from_raw(width as u32, height as u32, rgba)
                    .ok_or("image buffer size mismatch")?;
            let q = quality.clamp(1, 100) as u8;
            let fmt = if format == sf_settings::ImageFormat::Jpeg {
                image::ImageFormat::Jpeg
            } else {
                image::ImageFormat::WebP
            };
            // JPEG has no alpha; the capture is opaque so a straight encode is fine.
            let mut out = std::fs::File::create(path).map_err(|e| format!("create {path}: {e}"))?;
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, q);
            if fmt == image::ImageFormat::Jpeg {
                let rgb: image::RgbImage =
                    image::DynamicImage::ImageRgba8(img).to_rgb8();
                encoder
                    .encode_image(&rgb)
                    .map_err(|e| format!("jpeg encode: {e}"))?;
            } else {
                img.save_with_format(path, image::ImageFormat::WebP)
                    .map_err(|e| format!("webp encode: {e}"))?;
            }
            Ok(())
        }
    }
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
    // Palette samples are gathered by pixel stride into a small fixed buffer.
    // Holding whole frames here would pin ~350 MB at 4K (12 × 29 MB); the global
    // palette only needs a spread of colors, not full frames.
    let mut palette_samples: Vec<u8> = Vec::new();
    const MAX_SAMPLE_BYTES: usize = 6 * 1024 * 1024;
    let frame_bytes = (w * h * 4) as usize;
    let sample_stride = (frame_bytes / MAX_SAMPLE_BYTES).max(1);
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
            // Thin this frame into the palette sample buffer.
            if palette_samples.len() < MAX_SAMPLE_BYTES {
                let mut p = 0;
                while p + 3 < frame_bytes && palette_samples.len() + 3 < MAX_SAMPLE_BYTES {
                    palette_samples.extend_from_slice(&shot.bgra[p..p + 4]);
                    p += 4 * sample_stride;
                }
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
    gif.build_global_palette(&[palette_samples]);

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
fn run_tray(
    seconds: f64,
    modifiers: u32,
    vk: u32,
    out_dir: std::path::PathBuf,
    format: sf_settings::ImageFormat,
    quality: i32,
) -> Result<String, String> {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    let count = Arc::new(AtomicU32::new(0));
    // Save into the settings directory; fall back to temp if it cannot be created.
    let out_dir = if std::fs::create_dir_all(&out_dir).is_ok() {
        out_dir
    } else {
        std::env::temp_dir().join("sf-out")
    };
    let ext = match format {
        sf_settings::ImageFormat::Png => "png",
        sf_settings::ImageFormat::Jpeg => "jpg",
        sf_settings::ImageFormat::Webp => "webp",
    };
    let _ = quality; // PNG is lossless; JPEG/WebP quality is applied by write_image

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
                let path = dir_cb.join(format!("ScreenForge-{n}.{ext}"));
                if let Err(e) = sf_capture::copy_bgra_to_clipboard(shot.width, shot.height, &shot.bgra) {
                    eprintln!("hotkey {n}: clipboard failed: {e}");
                }
                match write_image(path.to_str().unwrap_or("out.png"), shot.width, shot.height, &shot.bgra, format, quality) {
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
                    let path = dir.join(format!("ScreenForge-tray-{n}.{ext}"));
                    let _ = write_image(path.to_str().unwrap_or("out.png"), shot.width, shot.height, &shot.bgra, format, quality);
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
fn run_tray(_seconds: f64, _modifiers: u32, _vk: u32, _out_dir: std::path::PathBuf, _format: sf_settings::ImageFormat, _quality: i32) -> Result<String, String> {
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

    write_by_extension(out, shot.width, shot.height, &shot.bgra)?;
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

/// Translates a settings hotkey's modifier flags into the Win32 `MOD_*` bits.
/// `sf_settings::Modifiers` already uses the same bit values, so this is a copy.
fn modifiers_from(hotkey: &sf_settings::HotkeyConfig) -> u32 {
    hotkey.modifiers.0 as u32
}

/// Maps a settings hotkey key name to a Win32 virtual-key code.
fn vk_from(hotkey: &sf_settings::HotkeyConfig) -> Option<u32> {
    let key = hotkey.key.trim();
    if key.is_empty() {
        return None;
    }

    // "D1".."D9" → '1'..'9'; "A".."Z" → ASCII; named keys otherwise.
    let bytes = key.as_bytes();
    if bytes.len() == 2 && (bytes[0] == b'D' || bytes[0] == b'd') && bytes[1].is_ascii_digit() {
        return Some(bytes[1] as u32);
    }
    if bytes.len() == 1 && bytes[0].is_ascii_uppercase() {
        return Some(bytes[0] as u32);
    }
    Some(match key {
        "Escape" => 0x1B,
        "Space" => 0x20,
        "Tab" => 0x09,
        "Return" | "Enter" => 0x0D,
        "F1" => 0x70,
        "F2" => 0x71,
        "F3" => 0x72,
        "F4" => 0x73,
        "F5" => 0x74,
        "F6" => 0x75,
        "F7" => 0x76,
        "F8" => 0x77,
        "F9" => 0x78,
        "F10" => 0x79,
        "F11" => 0x7A,
        "F12" => 0x7B,
        _ => return None,
    })
}
