//! Visual-translate language memory. Port of `ScreenForge/Settings/VisualTranslateRecents.cs`.
//!
//! The C# version kept the session override in a process-global static. Here the
//! session state is held by a [`VisualTranslateRecents`] value instead, so there
//! is no mutable global — the app owns one instance.

use crate::{default_pair, is_known, map_ui_culture, AppSettings};

pub const MAX: usize = 5;

#[derive(Default)]
pub struct VisualTranslateRecents {
    session_code: Option<String>,
}

impl VisualTranslateRecents {
    pub fn new() -> Self {
        Self::default()
    }

    /// The native language from settings, or the UI culture's language as a fallback.
    pub fn native_code(&self, settings: &AppSettings, ui_culture: &str) -> String {
        let code = settings.translate_native_language.trim().to_lowercase();
        if is_known(&code) {
            code
        } else {
            map_ui_culture(ui_culture)
        }
    }

    /// The language the visual-translate menu currently targets.
    pub fn active_code(&self, settings: &AppSettings, ui_culture: &str) -> String {
        if let Some(code) = &self.session_code {
            if !code.trim().is_empty() && is_known(code) {
                return code.clone();
            }
        }
        self.native_code(settings, ui_culture)
    }

    /// Sets the session override. A code equal to the native language clears it
    /// (so the native default is not replaced).
    pub fn set_session(&mut self, settings: &AppSettings, code: &str, ui_culture: &str) {
        let code = code.trim().to_lowercase();
        if !is_known(&code) {
            return;
        }

        self.session_code = if code.eq_ignore_ascii_case(&self.native_code(settings, ui_culture)) {
            None
        } else {
            Some(code)
        };
    }

    pub fn clear_session(&mut self) {
        self.session_code = None;
    }

    /// Records a chosen language in settings: newest first, native excluded, capped.
    pub fn remember(&self, settings: &mut AppSettings, code: &str, ui_culture: &str) {
        let code = code.trim().to_lowercase();
        let native = self.native_code(settings, ui_culture);

        settings.recent_visual_languages.retain(|c| {
            !c.eq_ignore_ascii_case(&code)
                && !c.eq_ignore_ascii_case(&native)
                && is_known(c)
        });

        if !code.eq_ignore_ascii_case(&native) && is_known(&code) {
            settings.recent_visual_languages.insert(0, code);
        }

        settings.recent_visual_languages.truncate(MAX);
    }

    /// Recents to show in the menu: known, not the active language, deduplicated, capped.
    pub fn visible(&self, settings: &AppSettings, ui_culture: &str) -> Vec<String> {
        let active = self.active_code(settings, ui_culture);
        let mut list: Vec<String> = Vec::new();

        for code in &settings.recent_visual_languages {
            let c = code.trim().to_lowercase();
            if !is_known(&c) || c.eq_ignore_ascii_case(&active) || list.contains(&c) {
                continue;
            }
            list.push(c);
            if list.len() == MAX {
                break;
            }
        }

        list
    }
}

/// Keeps the pair language derived from the native language consistent, matching
/// the C# normalize step that uses `DefaultPair`.
pub fn ensure_pair(settings: &mut AppSettings) {
    if settings.translate_pair_language.trim().is_empty() {
        settings.translate_pair_language = default_pair(&settings.translate_native_language);
    }
}
