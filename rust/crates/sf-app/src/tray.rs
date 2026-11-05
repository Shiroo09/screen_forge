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

    let count = Arc::new(AtomicU32::new(0));

    // Her kısayol kendi kaydını tutar: biri çakışsa bile diğerleri çalışır.
    // Ayar `is_valid()` ile doldurulmamışsa (modifiers boş) atlanır — boş bir
    // kısayolu kaydetmeye çalışmak RegisterHotKey'i boşuna dener.
    let mut regs: Vec<sf_capture::hotkey::HotkeyRegistration> = Vec::new();
    for (name, cfg, job) in [
        (
            "bölge",
            settings.region_hotkey.clone(),
            Job::Region,
        ),
        ("tam ekran", settings.full_screen_hotkey, Job::FullScreen),
        (
            "tam ekran + yükle",
            settings.full_screen_upload_hotkey,
            Job::FullScreenUpload,
        ),
        ("kolaj", settings.collage_hotkey, Job::Collage),
        (
            "hızlı çeviri",
            settings.quick_translate_hotkey,
            Job::Translate,
        ),
    ] {
        let modifiers = cfg.modifiers.0 as u32;
        // Atlanan her kısayol gerekçesiyle bildirilir: sessiz yutma, kullanıcıya
        // "ayarlarda aktif görünüyor ama çalışmıyor" dedirtiyordu.
        let Some(vk) = vk_from(&cfg) else {
            eprintln!("kısayol atlandı ({name}): tuş tanınmadı ({:?})", cfg.key);
            continue;
        };
        if modifiers == 0 {
            eprintln!("kısayol atlandı ({name}): değiştirici tuş yok");
            continue;
        }
        let (d, c, f, q) = (
            out_dir.clone(),
            Arc::clone(&count),
            settings.output_format,
            settings.quality,
        );
        match sf_capture::hotkey::register(modifiers, vk, move || run_job(job, &c, &d, f, q)) {
            Ok(r) => regs.push(r),
            Err(e) => eprintln!("kısayol kaydedilemedi ({name}): {e}"),
        }
    }

    if regs.is_empty() {
        eprintln!("hiçbir kısayol kaydedilemedi; tepsi menüsü yine çalışır");
    } else {
        eprintln!("{} kısayol kaydedildi", regs.len());
    }

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

        let (d, c, f, q) = (out_dir.clone(), Arc::clone(&count), settings.output_format, settings.quality);
        items.push((
            "Tam Ekran + Buluta Yükle".to_string(),
            Box::new(move || upload_fullscreen(&c, &d, f, q)),
        ));

        items.push((
            "Kolaj".to_string(),
            Box::new(|| open_window("collage")),
        ));

        items.push((
            "Renk Seçici".to_string(),
            Box::new(|| match sf_capture::color_picker::pick_color() {
                Some(rgb) => {
                    let hex = format!("#{rgb:06X}");
                    match sf_capture::color_picker::copy_hex_to_clipboard(&hex) {
                        Ok(()) => eprintln!("renk seçici: {hex} kopyalandı"),
                        Err(e) => eprintln!("renk seçici: {hex} · pano hatası: {e}"),
                    }
                }
                None => eprintln!("renk seçici: iptal"),
            }),
        ));
        items.push((
            "Hızlı Çeviri".to_string(),
            Box::new(|| open_window("translate")),
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
        let _ = (&regs, &tray);
    }
}

/// Kısayolların tetiklediği işler; menü öğeleriyle aynı gövdeyi paylaşır.
#[derive(Clone, Copy)]
enum Job {
    Region,
    FullScreen,
    FullScreenUpload,
    Collage,
    Translate,
}

fn run_job(
    job: Job,
    count: &AtomicU32,
    out_dir: &std::path::Path,
    format: sf_settings::ImageFormat,
    quality: i32,
) {
    match job {
        Job::Region => capture_once(count, out_dir, format, quality),
        Job::FullScreen => capture_fullscreen(count, out_dir, format, quality),
        // Tam ekran + yükle: yakala, kaydet, Litterbox'a gönder.
        Job::FullScreenUpload => upload_fullscreen(count, out_dir, format, quality),
        // Kolaj kısayolu pencereyi kolaj sekmesinde açar; kolaj birden çok
        // görsel ister, tek yakalamayla anlamı yok.
        Job::Collage => open_window("collage"),
        Job::Translate => open_window("translate"),
    }
}

/// Yükleme kısayolu: tam ekran yakala, diske yaz, bağlantıyı stdout'a bas.
fn upload_fullscreen(
    count: &AtomicU32,
    out_dir: &std::path::Path,
    format: sf_settings::ImageFormat,
    quality: i32,
) {
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
        Ok(()) => match sf_net::http_upload::upload_file(&path.to_string_lossy()) {
            Ok(url) => eprintln!("tam ekran {n}: yüklendi {url}"),
            Err(e) => eprintln!("tam ekran {n}: yükleme hatası: {e}"),
        },
        Err(e) => eprintln!("tam ekran {n}: hata: {e}"),
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

    // Shapes are in virtual-screen coords; shift into the crop. The overlay's
    // option bar picked the stroke width, so honour it here.
    {
        use sf_draw::canvas::Canvas;
        let mut canvas = Canvas::new(&mut shot.bgra, shot.width, shot.height);
        for shape in &edit.shapes {
            sf_draw::model::render_with(
                &mut canvas,
                &shape.translate(-(x as f32), -(y as f32)),
                edit.stroke_width,
            );
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

    use sf_capture::overlay_chrome::Action;
    match edit.action {
        // "Çevir" bu pencerede OCR akışı olmadığı için kaydedip pencereyi açar.
        Some(Action::Translate) => {
            if let Err(e) = write_image(&path, &shot, format, quality) {
                eprintln!("yakalama {n}: hata: {e}");
                return;
            }
            eprintln!("yakalama {n}: kaydedildi {} · çeviri için pencere açılıyor", path.display());
            open_window("translate");
        }
        // "Buluta yükle" diske yazıp Litterbox'a gönderir; bağlantı stdout'a düşer.
        Some(Action::Upload) => {
            if let Err(e) = write_image(&path, &shot, format, quality) {
                eprintln!("yakalama {n}: hata: {e}");
                return;
            }
            match sf_net::http_upload::upload_file(&path.to_string_lossy()) {
                Ok(url) => eprintln!("yakalama {n}: yüklendi {url}"),
                Err(e) => eprintln!("yakalama {n}: yükleme hatası: {e}"),
            }
        }
        // Kaydet ve Enter/araç çubuğu dışı bitişler aynı: diske yaz.
        Some(Action::Save) | Some(Action::Copy) | Some(Action::Cancel) | None => {
            match write_image(&path, &shot, format, quality) {
                Ok(()) => eprintln!("yakalama {n}: kaydedildi {}", path.display()),
                Err(e) => eprintln!("yakalama {n}: hata: {e}"),
            }
        }
    }
}

/// Launches `--window --tab <tab>` as a child process: the overlay's "Çevir"
/// action and the kolaj/çeviri kısayolları buradan pencereye iner.
fn open_window(tab: &str) {
    let Ok(exe) = std::env::current_exe() else { return };
    let _ = std::process::Command::new(exe)
        .args(["--window", "--tab", tab])
        .spawn();
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
    // Tek harf, büyük/küçük fark etmez: ayar dosyasına elle "s" yazılmış
    // olabilir ve `is_valid()` yalnız boş olup olmadığına bakıyor.
    if b.len() == 1 && b[0].is_ascii_alphabetic() {
        return Some(b[0].to_ascii_uppercase() as u32);
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
