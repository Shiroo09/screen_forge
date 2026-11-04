//! Tray mode — the app's default launch, matching the original ScreenForge.
//!
//! Registers the global capture hotkey and the tray icon; each trigger opens the
//! region overlay with the annotate toolbar, crops the selection, copies it to the
//! clipboard and writes a PNG. No main window is shown.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// Runs until the process is killed. The hotkey thread owns the message loop.
pub fn run_tray() {
    let (settings, _) = sf_settings::AppSettings::load(&sf_settings::settings_path());

    // Keep the HKCU Run entry in sync with the saved preference on every start,
    // so the "başlangıçta çalıştır" checkbox actually takes effect.
    let _ = sf_settings::set_startup_enabled(settings.launch_at_startup);

    let out_dir = if std::path::PathBuf::from(&settings.save_directory).exists() {
        std::path::PathBuf::from(&settings.save_directory)
    } else {
        std::env::temp_dir()
    };

    let mods = settings.region_hotkey.modifiers.0 as u32;
    let vk = vk_from(&settings.region_hotkey).unwrap_or(0x53);

    let count = Arc::new(AtomicU32::new(0));

    let reg = match sf_capture::hotkey::register(mods, vk, {
        let out_dir = out_dir.clone();
        let count = Arc::clone(&count);
        let format = settings.output_format;
        let quality = settings.quality;
        move || capture_once(&count, &out_dir, format, quality)
    }) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("kısayol kaydedilemedi: {e}");
            return;
        }
    };

    // Tray icon with the original menu. Left-click captures; right-click opens
    // the menu (same items as the C# app's TrayIconService).
    let menu_items: Vec<(String, Box<dyn FnMut() + Send>)> = {
        let mut items: Vec<(String, Box<dyn FnMut() + Send>)> = Vec::new();

        let (d, c, f, q) = (out_dir.clone(), Arc::clone(&count), settings.output_format, settings.quality);
        items.push((
            "Bölge Yakala".to_string(),
            Box::new(move || capture_once(&c, &d, f, q)),
        ));

        let (d, c, f, q) = (out_dir.clone(), Arc::clone(&count), settings.output_format, settings.quality);
        items.push((
            "Tam Ekran Yakala".to_string(),
            Box::new(move || capture_fullscreen(&c, &d, f, q)),
        ));

        items.push((
            "Renk Seçici".to_string(),
            Box::new(|| eprintln!("renk seçici: yakında")),
        ));
        items.push((
            "Hızlı Çeviri".to_string(),
            Box::new(|| eprintln!("hızlı çeviri: pencereyi aç (--window)")),
        ));
        items.push((
            "Ayarlar".to_string(),
            Box::new(|| {
                let (mut s, _) = sf_settings::AppSettings::load(&sf_settings::settings_path());
                let _ = sf_capture::settings_window::show(&mut s);
                let _ = s.save(&sf_settings::settings_path());
            }),
        ));
        items.push((
            "Çıkış".to_string(),
            Box::new(|| std::process::exit(0)),
        ));
        items
    };

    let tray = sf_capture::tray::install_with_menu(
        "ScreenForge — ekran görüntüsü",
        {
            let out_dir = out_dir.clone();
            let count = Arc::clone(&count);
            let format = settings.output_format;
            let quality = settings.quality;
            move || capture_once(&count, &out_dir, format, quality)
        },
        menu_items,
    )
    .ok();

    eprintln!(
        "ScreenForge tepside — {}. Görsel: {}",
        settings.region_hotkey.display(),
        out_dir.display()
    );

    // Park the main thread; hotkey/tray threads do the work.
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
        let _ = (&reg, &tray);
    }
}

/// One capture: overlay → annotate → crop → clipboard + file.
fn capture_once(
    count: &AtomicU32,
    out_dir: &std::path::Path,
    format: sf_settings::ImageFormat,
    quality: i32,
) {
    let n = count.fetch_add(1, Ordering::SeqCst) + 1;

    let Some(edit) = sf_capture::editor_window::edit_region() else {
        eprintln!("yakalama {n}: iptal");
        return;
    };

    let (x, y, w, h) = edit.region;
    let Ok(mut shot) = sf_capture::capture_region(x, y, w, h) else {
        eprintln!("yakalama {n}: bölge alınamadı");
        return;
    };

    // Shapes are in virtual-screen coords; shift into the crop.
    {
        use sf_draw::canvas::Canvas;
        let mut canvas = Canvas::new(&mut shot.bgra, shot.width, shot.height);
        for shape in &edit.shapes {
            sf_draw::model::render(&mut canvas, &shape.translate(-(x as f32), -(y as f32)));
        }
    }

    if let Err(e) = sf_capture::copy_bgra_to_clipboard(shot.width, shot.height, &shot.bgra) {
        eprintln!("yakalama {n}: pano hatası: {e}");
    }

    let ext = match format {
        sf_settings::ImageFormat::Png => "png",
        sf_settings::ImageFormat::Jpeg => "jpg",
        sf_settings::ImageFormat::Webp => "webp",
    };
    let path = out_dir.join(format!("ScreenForge-{n}.{ext}"));
    match write_image(&path, &shot, format, quality) {
        Ok(()) => eprintln!("yakalama {n}: kaydedildi {}", path.display()),
        Err(e) => eprintln!("yakalama {n}: hata: {e}"),
    }
}

/// Full-screen capture (no region picker): grab everything, save + clipboard.
fn capture_fullscreen(
    count: &AtomicU32,
    out_dir: &std::path::Path,
    format: sf_settings::ImageFormat,
    quality: i32,
) {
    let _ = quality; // PNG is lossless; the encoder applies quality for JPEG/WebP
    let n = count.fetch_add(1, Ordering::SeqCst) + 1;
    let Ok(shot) = sf_capture::capture_virtual_screen() else {
        eprintln!("tam ekran {n}: yakalanamadı");
        return;
    };
    let _ = sf_capture::copy_bgra_to_clipboard(shot.width, shot.height, &shot.bgra);
    let ext = match format {
        sf_settings::ImageFormat::Png => "png",
        sf_settings::ImageFormat::Jpeg => "jpg",
        sf_settings::ImageFormat::Webp => "webp",
    };
    let path = out_dir.join(format!("ScreenForge-tam-{n}.{ext}"));
    match write_image(&path, &shot, format, quality) {
        Ok(()) => eprintln!("tam ekran {n}: kaydedildi {}", path.display()),
        Err(e) => eprintln!("tam ekran {n}: hata: {e}"),
    }
}

fn write_image(
    path: &std::path::Path,
    shot: &sf_capture::Captured,
    format: sf_settings::ImageFormat,
    quality: i32,
) -> Result<(), String> {
    let mut rgba = Vec::with_capacity(shot.bgra.len());
    for px in shot.bgra.chunks_exact(4) {
        rgba.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
    }
    let img = image::RgbaImage::from_raw(shot.width as u32, shot.height as u32, rgba)
        .ok_or("boyut uyuşmadı")?;
    let q = quality.clamp(1, 100) as u8;
    match format {
        sf_settings::ImageFormat::Png => img.save(path).map_err(|e| e.to_string()),
        sf_settings::ImageFormat::Jpeg => {
            let rgb = image::DynamicImage::ImageRgba8(img).to_rgb8();
            let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
            image::codecs::jpeg::JpegEncoder::new_with_quality(file, q)
                .encode_image(&rgb)
                .map_err(|e| e.to_string())
        }
        sf_settings::ImageFormat::Webp => img
            .save_with_format(path, image::ImageFormat::WebP)
            .map_err(|e| e.to_string()),
    }
}

/// Maps a settings hotkey key name to a Win32 virtual-key code.
fn vk_from(hotkey: &sf_settings::HotkeyConfig) -> Option<u32> {
    let key = hotkey.key.trim();
    if key.is_empty() {
        return None;
    }
    let b = key.as_bytes();
    if b.len() == 2 && (b[0] == b'D' || b[0] == b'd') && b[1].is_ascii_digit() {
        return Some(b[1] as u32);
    }
    if b.len() == 1 && b[0].is_ascii_uppercase() {
        return Some(b[0] as u32);
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
