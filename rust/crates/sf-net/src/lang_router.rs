//! Picks the second-pass target for Spotlight translate.
//! Port of `ScreenForge/Translate/TranslateLanguageRouter.cs`.

/// First call is always auto → native. If the source is already native (or the
/// first result is an identity with no reported source), translate to the pair.
pub fn should_translate_to_pair(
    native_language: &str,
    source_lang: &str,
    original_text: &str,
    first_translation: &str,
) -> bool {
    if languages_equal(source_lang, native_language) {
        return true;
    }

    source_lang.trim().is_empty() && is_identity(original_text, first_translation)
}

pub fn languages_equal(a: &str, b: &str) -> bool {
    if a.trim().is_empty() || b.trim().is_empty() {
        return false;
    }

    let a = a.trim();
    let b = b.trim();
    if a.eq_ignore_ascii_case(b) {
        return true;
    }

    primary_code(a).eq_ignore_ascii_case(primary_code(b))
}

/// Image translation: source is always auto, target is always the native language.
pub fn image_route(native_language: Option<&str>) -> (String, Option<String>) {
    let target = match native_language {
        Some(l) if !l.trim().is_empty() => l.trim().to_string(),
        _ => "tr".to_string(),
    };
    (target, None)
}

fn is_identity(original: &str, translated: &str) -> bool {
    original.trim() == translated.trim()
}

fn primary_code(code: &str) -> &str {
    match code.find('-') {
        Some(dash) if dash > 0 => &code[..dash],
        _ => code,
    }
}
