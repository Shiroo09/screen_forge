//! Known UI languages plus first-run native/pair defaults.
//! Port of `ScreenForge/Settings/TranslateLanguageDefaults.cs`.

pub const LANGUAGES: &[(&str, &str)] = &[
    ("tr", "Türkçe"),
    ("en", "English"),
    ("de", "Deutsch"),
    ("fr", "Français"),
    ("es", "Español"),
    ("it", "Italiano"),
    ("pt", "Português"),
    ("ru", "Русский"),
    ("ar", "العربية"),
    ("zh", "中文"),
    ("ja", "日本語"),
    ("ko", "한국어"),
    ("nl", "Nederlands"),
    ("pl", "Polski"),
    ("uk", "Українська"),
    ("hi", "हिन्दी"),
];

pub fn label(code: &str) -> String {
    let code = code.trim().to_lowercase();
    for (c, label) in LANGUAGES {
        if *c == code {
            return (*label).to_string();
        }
    }
    code
}

pub fn is_known(code: &str) -> bool {
    let code = code.trim().to_lowercase();
    LANGUAGES.iter().any(|(c, _)| *c == code)
}

/// Maps a two-letter UI culture name to a supported code, defaulting to "en".
pub fn map_ui_culture(two_letter: &str) -> String {
    let two = two_letter.to_lowercase();
    if is_known(&two) {
        two
    } else {
        "en".to_string()
    }
}

/// The pair language for a given native language: "tr" -> "en", everything else -> "en".
pub fn default_pair(native_language: &str) -> String {
    if native_language.trim().eq_ignore_ascii_case("en") {
        "tr".to_string()
    } else {
        "en".to_string()
    }
}
