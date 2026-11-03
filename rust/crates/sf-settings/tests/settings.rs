//! Settings port contract:
//! - read the JSON the C# app writes (fixture from `GoldenSettingsFixture.cs`)
//! - write JSON the C# app reads back with the same values
//! - atomic save, corrupt-file backup, first-run defaults

use sf_settings::{AppSettings, HotkeyConfig, ImageFormat, Modifiers, VideoQuality};

fn fixture_json() -> String {
    let path = format!(
        "{}/tests/fixtures/golden_settings.json",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {path}: {e} — run SF_WRITE_GOLDEN=1 dotnet test"))
}

#[test]
fn reads_csharp_written_settings() {
    let settings = AppSettings::deserialize(&fixture_json()).expect("C# settings must parse");

    // Flags enum survives the "Alt, Control" wire form.
    assert_eq!(settings.region_hotkey.key, "S");
    assert!(settings.region_hotkey.modifiers.contains(Modifiers::ALT));
    assert!(settings.region_hotkey.modifiers.contains(Modifiers::CONTROL));
    assert!(settings.region_hotkey.is_valid());

    // Quick translate hotkey: Ctrl + D1.
    assert_eq!(settings.quick_translate_hotkey.key, "D1");
    assert!(settings.quick_translate_hotkey.modifiers.contains(Modifiers::CONTROL));
    assert!(!settings.quick_translate_hotkey.modifiers.contains(Modifiers::ALT));

    // String enums.
    assert_eq!(settings.output_format, ImageFormat::Webp);
    assert_eq!(settings.video.quality, VideoQuality::Medium);
    assert_eq!(settings.gif.quantizer, "Octree");

    // Scalar and nested values.
    assert!(settings.show_cursor);
    assert!(!settings.auto_copy_link_after_upload);
    assert_eq!(settings.quality, 88);
    assert_eq!(settings.save_directory, "C:/shots");
    assert_eq!(settings.gif.fps, 24);
    assert_eq!(settings.gif.color_count, 128);
    assert!(settings.gif.use_global_palette);
    assert_eq!(settings.video.fps, 25);
    assert_eq!(settings.presenter.pen_width, 7.0);
    assert_eq!(settings.presenter.spotlight_radius, 300.0);
    assert_eq!(settings.subtitle.theme, "gunduz");
    assert_eq!(settings.subtitle.opacity, 0.5);
    assert_eq!(settings.recent_visual_languages, vec!["de", "fr"]);
    assert_eq!(settings.tool_styles.text_alignment as u8, 1); // Center
    assert_eq!(settings.tool_styles.step_shape as u8, 2); // Bubble
}

#[test]
fn round_trips_through_json() {
    let original = AppSettings::deserialize(&fixture_json()).unwrap();
    let json = original.to_json();
    let reloaded = AppSettings::deserialize(&json).expect("Rust-written JSON must parse back");

    assert_eq!(original, reloaded);
}

#[test]
fn writes_csharp_compatible_field_names_and_flag_string() {
    let mut flags = Modifiers::empty();
    flags.insert(Modifiers::ALT);
    flags.insert(Modifiers::CONTROL);
    let settings = AppSettings {
        region_hotkey: HotkeyConfig::new(flags, "S"),
        ..AppSettings::default()
    };

    let json = settings.to_json();

    // PascalCase keys and the ", "-joined flag form the C# converter produces.
    assert!(json.contains("\"RegionHotkey\""), "missing PascalCase key: {json}");
    assert!(json.contains("\"Modifiers\": \"Alt, Control\""), "bad flag form: {json}");
    assert!(json.contains("\"OutputFormat\": \"Png\""), "enum must be a name: {json}");
    assert!(json.contains("\"IsValid\": true"), "computed IsValid must be written: {json}");
}

#[test]
fn save_is_atomic_and_leaves_no_temp_files() {
    let dir = std::env::temp_dir().join(format!("sf-settings-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");

    AppSettings::default().save(&path).unwrap();
    let s = AppSettings {
        show_cursor: true,
        ..AppSettings::default()
    };
    s.save(&path).unwrap();

    assert!(path.exists());
    let leftovers: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "temp files left behind: {leftovers:?}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_corrupt_file_keeps_backup_and_returns_defaults() {
    let dir = std::env::temp_dir().join(format!("sf-settings-corrupt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");
    std::fs::write(&path, "{ bu geçerli bir JSON değil ").unwrap();

    let (loaded, is_first_run) = AppSettings::load(&path);

    assert!(is_first_run);
    assert!(loaded.region_hotkey.is_valid());
    assert_eq!(loaded.region_hotkey.key, "S");

    let mut backup = path.as_os_str().to_os_string();
    backup.push(".corrupt");
    assert!(std::path::Path::new(&backup).exists(), "corrupt backup missing");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_missing_file_starts_fresh() {
    let dir = std::env::temp_dir().join(format!("sf-settings-missing-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");

    let (loaded, is_first_run) = AppSettings::load(&path);

    assert!(is_first_run);
    assert!(loaded.region_hotkey.is_valid());

    let mut backup = path.as_os_str().to_os_string();
    backup.push(".corrupt");
    assert!(!std::path::Path::new(&backup).exists());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn normalize_clamps_and_migrates() {
    let mut s = AppSettings::default();
    s.gif.fps = 900;
    s.video.fps = -5;
    s.settings_revision = 0;
    s.region_hotkey = HotkeyConfig::default(); // invalid (empty key)
    s.translate_native_language = String::new();
    s.translate_pair_language = String::new();
    #[allow(clippy::field_reassign_with_default)]
    let _ = &s;

    s.normalize("tr");

    assert_eq!(s.gif.fps, 60);
    assert_eq!(s.video.fps, 1);
    assert_eq!(s.settings_revision, 2);
    assert_eq!(s.region_hotkey.key, "S"); // migrated to default
    assert_eq!(s.translate_native_language, "tr");
    assert_eq!(s.translate_pair_language, "en");
}

#[test]
fn wire_form_round_trips_every_flag_combination() {
    for bits in 0u8..16 {
        let m = Modifiers(bits);
        let wire = m.to_wire();
        assert_eq!(Modifiers::from_wire(&wire), m, "wire {wire:?} for bits {bits}");
    }
}

// ─── Visual translate recents ─────────────────────────────────────────────────

use sf_settings::VisualTranslateRecents;

fn tr_settings() -> AppSettings {
    AppSettings {
        translate_native_language: "tr".into(),
        translate_pair_language: "en".into(),
        ..AppSettings::default()
    }
}

#[test]
fn active_code_uses_native_not_pair() {
    let r = VisualTranslateRecents::new();
    assert_eq!(r.active_code(&tr_settings(), "tr"), "tr");
}

#[test]
fn set_session_overrides_until_cleared() {
    let mut r = VisualTranslateRecents::new();
    let s = tr_settings();

    r.set_session(&s, "de", "tr");
    assert_eq!(r.active_code(&s, "tr"), "de");

    r.set_session(&s, "tr", "tr");
    assert_eq!(r.active_code(&s, "tr"), "tr");
}

#[test]
fn remember_skips_native_and_puts_latest_first() {
    let r = VisualTranslateRecents::new();
    let mut s = tr_settings();

    r.remember(&mut s, "en", "tr");
    r.remember(&mut s, "de", "tr");
    r.remember(&mut s, "fr", "tr");

    assert_eq!(r.visible(&s, "tr"), vec!["fr", "de", "en"]);
}

#[test]
fn visible_hides_recent_that_matches_active() {
    let mut r = VisualTranslateRecents::new();
    let mut s = tr_settings();
    s.recent_visual_languages = vec!["de".into(), "fr".into()];

    r.set_session(&s, "de", "tr");
    assert_eq!(r.visible(&s, "tr"), vec!["fr"]);
}

#[test]
fn remember_caps_at_five() {
    let r = VisualTranslateRecents::new();
    let mut s = tr_settings();

    for code in ["de", "fr", "es", "it", "pt", "ru"] {
        r.remember(&mut s, code, "tr");
    }

    assert_eq!(s.recent_visual_languages.len(), 5);
    assert_eq!(s.recent_visual_languages[0], "ru");
    assert!(!s.recent_visual_languages.contains(&"de".to_string()));
}
