//! Windows-reserved keyboard shortcuts. Port of `ScreenForge/Hotkeys/WindowsReservedHotkeys.cs`.
//!
//! Assignment is not blocked; the caller gets a warning string instead.

use sf_settings::{HotkeyConfig, Modifiers};

/// Returns the warning for a reserved chord, or `None` when the chord is free.
pub fn describe(config: &HotkeyConfig) -> Option<String> {
    if !config.is_valid() {
        return None;
    }

    let key = normalize_key(&config.key);
    let reason = lookup(config.modifiers.0, &key)?;
    Some(format!("Windows kullanıyor: {reason}"))
}

fn lookup(mods: u8, key: &str) -> Option<&'static str> {
    use Modifiers as M;

    // Win+D0..D9 → taskbar.
    if mods == M::WINDOWS && key.len() == 2 && key.starts_with('D') && key[1..].chars().all(|c| c.is_ascii_digit()) {
        return Some("Görev çubuğu");
    }

    let entry = match (mods, key) {
        (m, "Tab") if m == M::ALT => "Görev değiştirme",
        (m, "Escape") if m == M::ALT => "Pencere döngüsü",
        (m, "F4") if m == M::ALT => "Pencereyi kapat",
        (m, "Space") if m == M::ALT => "Pencere menüsü",
        (m, "Snapshot") if m == M::ALT => "Pencere görüntüsü",
        (m, "Snapshot") if m == M::NONE => "Ekran görüntüsü",
        (m, "Escape") if m == M::CONTROL => "Başlat menüsü",
        (m, "Escape") if m == M::CONTROL | M::SHIFT => "Görev yöneticisi",
        (m, "Delete") if m == M::CONTROL | M::ALT => "Güvenlik ekranı",
        (m, "D") if m == M::WINDOWS => "Masaüstünü göster",
        (m, "E") if m == M::WINDOWS => "Dosya Gezgini",
        (m, "L") if m == M::WINDOWS => "Kilitle",
        (m, "R") if m == M::WINDOWS => "Çalıştır",
        (m, "Tab") if m == M::WINDOWS => "Görev görünümü",
        (m, "I") if m == M::WINDOWS => "Ayarlar",
        (m, "S") if m == M::WINDOWS => "Arama",
        (m, "A") if m == M::WINDOWS => "Hızlı ayarlar",
        (m, "X") if m == M::WINDOWS => "Hızlı bağlantı",
        (m, "M") if m == M::WINDOWS => "Simge durumuna küçült",
        (m, "P") if m == M::WINDOWS => "Yansıt",
        (m, "V") if m == M::WINDOWS => "Pano",
        (m, "G") if m == M::WINDOWS => "Oyun çubuğu",
        (m, "K") if m == M::WINDOWS => "Bağlan",
        (m, "H") if m == M::WINDOWS => "Sesle yazma",
        (m, "W") if m == M::WINDOWS => "Pencere öğeleri",
        (m, "Z") if m == M::WINDOWS => "Yapışkan düzenler",
        (m, "U") if m == M::WINDOWS => "Erişilebilirlik",
        (m, "Pause") if m == M::WINDOWS => "Sistem",
        (m, "Snapshot") if m == M::WINDOWS => "Ekran görüntüsü kaydet",
        (m, "Up") if m == M::WINDOWS => "Pencere yerleştirme",
        (m, "Down") if m == M::WINDOWS => "Pencere yerleştirme",
        (m, "Left") if m == M::WINDOWS => "Pencere yerleştirme",
        (m, "Right") if m == M::WINDOWS => "Pencere yerleştirme",
        (m, "S") if m == M::WINDOWS | M::SHIFT => "Ekran alıntısı",
        (m, "M") if m == M::WINDOWS | M::SHIFT => "Pencereleri geri yükle",
        (m, "D") if m == M::WINDOWS | M::CONTROL => "Yeni masaüstü",
        (m, "Left") if m == M::WINDOWS | M::CONTROL => "Masaüstü değiştir",
        (m, "Right") if m == M::WINDOWS | M::CONTROL => "Masaüstü değiştir",
        (m, "OemPeriod") if m == M::WINDOWS => "Emoji",
        _ => return None,
    };
    Some(entry)
}

fn normalize_key(key: &str) -> String {
    let key = key.trim();
    if key.eq_ignore_ascii_case("PrintScreen") || key.eq_ignore_ascii_case("PrtSc") {
        "Snapshot".to_string()
    } else {
        key.to_string()
    }
}
