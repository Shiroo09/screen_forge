//! Application settings. Port of `ScreenForge/Settings/AppSettings.cs`.
//!
//! Contract: the Rust model must *read* the JSON the C# app writes (PascalCase
//! keys, string enums, `"Alt, Control"` flag strings) and *write* JSON the C#
//! app reads back with the same values. Byte-identical files are not a goal —
//! C# writes `1` where serde_json writes `1.0` — semantic equivalence is.

mod lang;
mod modifiers;
mod recents;

pub use lang::{default_pair, is_known, label, map_ui_culture, LANGUAGES};
pub use modifiers::Modifiers;
pub use recents::{ensure_pair, VisualTranslateRecents};

use serde::{Deserialize, Serialize};
use std::path::Path;

// ─── Enums ────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum ImageFormat {
    #[default]
    Png,
    Jpeg,
    Webp,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum VideoQuality {
    Low,
    #[default]
    Medium,
    High,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum StepShape {
    #[default]
    Circle,
    Square,
    Bubble,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum TextAlignmentMode {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum InkShapeConvertMode {
    #[default]
    DrawAndHold,
    Auto,
    Disabled,
}

// ─── Hotkey ───────────────────────────────────────────────────────────────────

/// A single global-hotkey definition. The C# type serializes a computed
/// `IsValid` alongside the stored fields, so both are emitted here.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct HotkeyConfig {
    pub modifiers: Modifiers,
    pub key: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct HotkeyWire {
    modifiers: Modifiers,
    key: String,
    #[serde(default)]
    is_valid: bool,
}

impl HotkeyConfig {
    pub fn new(modifiers: Modifiers, key: &str) -> Self {
        Self {
            modifiers,
            key: key.to_string(),
        }
    }

    /// Alt+Shift+<key>, the default for the capture/translate hotkeys.
    pub fn alt_shift(key: &str) -> Self {
        let mut m = Modifiers::empty();
        m.insert(Modifiers::ALT);
        m.insert(Modifiers::SHIFT);
        Self::new(m, key)
    }

    /// Ctrl(+Shift)+<key>, the ZoomIt-style presenter defaults.
    pub fn ctrl(key: &str, shift: bool) -> Self {
        let mut m = Modifiers::empty();
        m.insert(Modifiers::CONTROL);
        if shift {
            m.insert(Modifiers::SHIFT);
        }
        Self::new(m, key)
    }

    pub fn is_valid(&self) -> bool {
        !self.key.trim().is_empty()
    }

    pub fn copy_from(&mut self, other: &HotkeyConfig) {
        self.key = other.key.clone();
        self.modifiers = other.modifiers;
    }

    /// Human-readable form for the UI, e.g. "Ctrl + Alt + S".
    pub fn display(&self) -> String {
        if !self.is_valid() {
            return "(atanmadı)".to_string();
        }
        let mut parts: Vec<&str> = Vec::new();
        if self.modifiers.contains(Modifiers::CONTROL) {
            parts.push("Ctrl");
        }
        if self.modifiers.contains(Modifiers::ALT) {
            parts.push("Alt");
        }
        if self.modifiers.contains(Modifiers::SHIFT) {
            parts.push("Shift");
        }
        if self.modifiers.contains(Modifiers::WINDOWS) {
            parts.push("Win");
        }
        let key = display_key(&self.key);
        parts.push(&key);
        parts.join(" + ")
    }
}

/// "D1".."D9" render as the bare digit, matching the C# `DisplayKey`.
fn display_key(key: &str) -> String {
    let bytes = key.as_bytes();
    if bytes.len() == 2 && (bytes[0] == b'D' || bytes[0] == b'd') && bytes[1].is_ascii_digit() {
        (bytes[1] as char).to_string()
    } else {
        key.to_string()
    }
}

impl Serialize for HotkeyConfig {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        HotkeyWire {
            modifiers: self.modifiers,
            key: self.key.clone(),
            is_valid: self.is_valid(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for HotkeyConfig {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = HotkeyWire::deserialize(deserializer)?;
        Ok(Self {
            modifiers: wire.modifiers,
            key: wire.key,
        })
    }
}

// ─── Tool style memory ────────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ToolStyleMemory {
    pub stroke_color: String,
    pub fill_color: String,
    pub free_background_color: String,
    pub stroke_width: f64,
    pub opacity: f64,
    pub arrow_head_scale: f64,
    pub font_family: String,
    pub font_size: f64,
    pub font_bold: bool,
    pub font_italic: bool,
    pub text_color: String,
    pub text_shadow: bool,
    pub text_shadow_level: i32,
    pub text_stroke: bool,
    pub text_stroke_color: String,
    pub text_ribbon: bool,
    pub text_ribbon_color: String,
    pub text_alignment: TextAlignmentMode,
    pub step_shape: StepShape,
    pub step_color: String,
    pub step_text_color: String,
    pub step_size: f64,
    pub blur_strength: f64,
    pub blur_pixelate: bool,
    pub last_tool: String,
}

impl Default for ToolStyleMemory {
    fn default() -> Self {
        Self {
            stroke_color: "#FFEA6F12".into(),
            fill_color: "#00000000".into(),
            free_background_color: "#FF1F2430".into(),
            stroke_width: 4.0,
            opacity: 1.0,
            arrow_head_scale: 1.0,
            font_family: "Segoe UI".into(),
            font_size: 20.0,
            font_bold: true,
            font_italic: false,
            text_color: "#FFFFFFFF".into(),
            text_shadow: true,
            text_shadow_level: 1,
            text_stroke: false,
            text_stroke_color: "#FF000000".into(),
            text_ribbon: true,
            text_ribbon_color: "#CC1F2430".into(),
            text_alignment: TextAlignmentMode::Left,
            step_shape: StepShape::Circle,
            step_color: "#FFE5484D".into(),
            step_text_color: "#FFFFFFFF".into(),
            step_size: 28.0,
            blur_strength: 8.0,
            blur_pixelate: false,
            last_tool: "Select".into(),
        }
    }
}

// ─── GIF / video / presenter / subtitle ───────────────────────────────────────

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct GifSettings {
    pub enabled: bool,
    pub fps: i32,
    pub color_count: i32,
    pub quantizer: String,
    pub sampling_factor: i32,
    pub dithering: bool,
    pub use_global_palette: bool,
    pub optimize_unchanged_pixels: bool,
    pub change_tolerance: i32,
    pub max_frame_memory_mb: i32,
    pub capture_cursor: bool,
    pub highlight_clicks: bool,
    pub highlight_cursor: bool,
    pub show_keys: bool,
    pub highlight_radius: f64,
}

impl Default for GifSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            fps: 20,
            color_count: 256,
            quantizer: "Neural".into(),
            sampling_factor: 5,
            dithering: false,
            use_global_palette: false,
            optimize_unchanged_pixels: true,
            change_tolerance: 0,
            max_frame_memory_mb: 1024,
            capture_cursor: true,
            highlight_clicks: true,
            highlight_cursor: false,
            show_keys: true,
            highlight_radius: 12.0,
        }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct VideoSettings {
    pub enabled: bool,
    pub fps: i32,
    pub quality: VideoQuality,
    pub capture_cursor: bool,
    pub highlight_clicks: bool,
    pub record_system_audio: bool,
    pub record_microphone: bool,
    pub mic_device_id: String,
    pub show_countdown: bool,
}

impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            fps: 30,
            quality: VideoQuality::High,
            capture_cursor: true,
            highlight_clicks: true,
            record_system_audio: true,
            record_microphone: false,
            mic_device_id: String::new(),
            show_countdown: true,
        }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SubtitleSettings {
    pub enabled: bool,
    pub source_x: i32,
    pub source_y: i32,
    pub source_w: i32,
    pub source_h: i32,
    pub box_x: i32,
    pub box_y: i32,
    pub box_w: i32,
    pub box_h: i32,
    pub theme: String,
    pub back_color: String,
    pub text_color: String,
    pub border_color: String,
    pub opacity: f64,
    pub border_thickness: f64,
}

impl Default for SubtitleSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            source_x: 0,
            source_y: 0,
            source_w: 0,
            source_h: 0,
            box_x: 0,
            box_y: 0,
            box_w: 0,
            box_h: 0,
            theme: "gece".into(),
            back_color: "#101218".into(),
            text_color: "#FFFFFF".into(),
            border_color: "#FFFFFF".into(),
            opacity: 0.82,
            border_thickness: 0.0,
        }
    }
}

impl SubtitleSettings {
    pub fn has_source(&self) -> bool {
        self.source_w >= 8 && self.source_h >= 8
    }

    pub fn has_box(&self) -> bool {
        self.box_w >= 40 && self.box_h >= 24
    }

    pub fn set_source(&mut self, x: i32, y: i32, w: i32, h: i32) {
        self.source_x = x;
        self.source_y = y;
        self.source_w = w;
        self.source_h = h;
    }

    pub fn set_box(&mut self, x: i32, y: i32, w: i32, h: i32) {
        self.box_x = x;
        self.box_y = y;
        self.box_w = w;
        self.box_h = h;
    }

    pub fn normalize(&mut self) {
        if self.source_w < 0 {
            self.source_w = 0;
        }
        if self.source_h < 0 {
            self.source_h = 0;
        }
        if self.box_w < 0 {
            self.box_w = 0;
        }
        if self.box_h < 0 {
            self.box_h = 0;
        }
        self.opacity = self.opacity.clamp(0.15, 1.0);
        self.border_thickness = self.border_thickness.clamp(0.0, 4.0);
        if self.theme.trim().is_empty() {
            self.theme = "gece".into();
        }
        if self.back_color.trim().is_empty() {
            self.back_color = "#101218".into();
        }
        if self.text_color.trim().is_empty() {
            self.text_color = "#FFFFFF".into();
        }
        if self.border_color.trim().is_empty() {
            self.border_color = "#FFFFFF".into();
        }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PresenterColorBind {
    pub color: String,
    pub hotkey: HotkeyConfig,
}

impl Default for PresenterColorBind {
    fn default() -> Self {
        Self {
            color: "#FFEA6F12".into(),
            hotkey: HotkeyConfig::default(),
        }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PresenterSettings {
    pub defaults_revision: i32,
    pub enabled: bool,
    pub pen_enabled: bool,
    pub pen_hotkey: HotkeyConfig,
    pub laser_enabled: bool,
    pub laser_hotkey: HotkeyConfig,
    pub rectangle_enabled: bool,
    pub rectangle_hotkey: HotkeyConfig,
    pub ellipse_enabled: bool,
    pub ellipse_hotkey: HotkeyConfig,
    pub arrow_enabled: bool,
    pub arrow_hotkey: HotkeyConfig,
    pub line_enabled: bool,
    pub line_hotkey: HotkeyConfig,
    pub spotlight_enabled: bool,
    pub spotlight_hotkey: HotkeyConfig,
    pub arrow_bend_enabled: bool,
    pub arrow_bend_hotkey: HotkeyConfig,
    pub cancel_enabled: bool,
    pub cancel_hotkey: HotkeyConfig,
    pub pen_color: String,
    pub ink_colors: Vec<String>,
    pub pen_width: f64,
    pub laser_color: String,
    pub laser_width: f64,
    pub laser_fade_ms: i32,
    pub spotlight_radius: f64,
    pub spotlight_softness: f64,
    pub spotlight_dim: f64,
    pub ink_shape_convert: InkShapeConvertMode,
    pub ink_text_convert_enabled: bool,
    pub color_binds: Vec<PresenterColorBind>,
}

pub const DEFAULT_INK_COLORS: [&str; 5] = [
    "#FFEA6F12",
    "#FF2563EB",
    "#FF16A34A",
    "#FFDC2626",
    "#FFFFFFFF",
];

impl Default for PresenterSettings {
    fn default() -> Self {
        Self {
            defaults_revision: 0,
            enabled: true,
            pen_enabled: false,
            pen_hotkey: HotkeyConfig::default(),
            laser_enabled: false,
            laser_hotkey: HotkeyConfig::default(),
            rectangle_enabled: false,
            rectangle_hotkey: HotkeyConfig::default(),
            ellipse_enabled: false,
            ellipse_hotkey: HotkeyConfig::default(),
            arrow_enabled: false,
            arrow_hotkey: HotkeyConfig::default(),
            line_enabled: false,
            line_hotkey: HotkeyConfig::default(),
            spotlight_enabled: false,
            spotlight_hotkey: HotkeyConfig::default(),
            arrow_bend_enabled: false,
            arrow_bend_hotkey: HotkeyConfig::default(),
            cancel_enabled: true,
            cancel_hotkey: HotkeyConfig::new(Modifiers::empty(), "Escape"),
            pen_color: "#FFEA6F12".into(),
            ink_colors: Vec::new(),
            pen_width: 5.0,
            laser_color: "#FFFF3D6E".into(),
            laser_width: 2.0,
            laser_fade_ms: 700,
            spotlight_radius: 200.0,
            spotlight_softness: 20.0,
            spotlight_dim: 0.40,
            ink_shape_convert: InkShapeConvertMode::DrawAndHold,
            ink_text_convert_enabled: true,
            color_binds: Vec::new(),
        }
    }
}

impl PresenterSettings {
    pub fn normalize(&mut self) {
        self.pen_width = self.pen_width.clamp(1.0, 24.0);
        self.laser_width = self.laser_width.clamp(1.0, 24.0);
        self.laser_fade_ms = self.laser_fade_ms.clamp(150, 3000);
        self.spotlight_radius = self.spotlight_radius.clamp(40.0, 600.0);
        self.spotlight_softness = self.spotlight_softness.clamp(8.0, 80.0);
        self.spotlight_dim = self.spotlight_dim.clamp(0.15, 0.85);
        if self.defaults_revision < 5 {
            self.apply_zoom_it_defaults();
        }
        if self.pen_color.trim().is_empty() {
            self.pen_color = "#FFEA6F12".into();
        }
        self.ensure_ink_colors();
        if self.laser_color.trim().is_empty() {
            self.laser_color = "#FFFF3D6E".into();
        }
        self.cancel_enabled = true;
        if !self.cancel_hotkey.is_valid() {
            self.cancel_hotkey = HotkeyConfig::new(Modifiers::empty(), "Escape");
        }
        if !self.arrow_bend_hotkey.is_valid() {
            self.arrow_bend_hotkey = HotkeyConfig::new(Modifiers::empty(), "Space");
        }
        if self.is_old_spotlight_factory() {
            self.spotlight_radius = 200.0;
            self.spotlight_softness = 20.0;
            self.spotlight_dim = 0.40;
        }
        if (self.laser_width - 6.0).abs() < 0.01
            && self.laser_color.eq_ignore_ascii_case("#FFE5484D")
            || self.laser_color.eq_ignore_ascii_case("#CCE5484D")
            || self.laser_color.eq_ignore_ascii_case("#FFE5484D")
        {
            if (self.laser_width - 6.0).abs() < 0.01 {
                self.laser_width = 3.0;
            }
            self.laser_color = "#FFFF3D6E".into();
        }
        for c in &mut self.color_binds {
            if c.color.trim().is_empty() {
                c.color = "#FFEA6F12".into();
            }
        }
    }

    fn is_old_spotlight_factory(&self) -> bool {
        self.spotlight_softness > 50.0
            || near(self.spotlight_radius, 155.0)
                && near(self.spotlight_softness, 28.0)
                && near(self.spotlight_dim, 0.52)
            || near(self.spotlight_radius, 155.0)
                && near(self.spotlight_softness, 175.0)
                && near(self.spotlight_dim, 0.52)
            || near(self.spotlight_radius, 180.0)
                && near(self.spotlight_softness, 90.0)
                && near(self.spotlight_dim, 0.55)
            || near(self.spotlight_radius, 120.0)
                && near(self.spotlight_softness, 36.0)
                && near(self.spotlight_dim, 0.64)
    }

    pub fn apply_zoom_it_defaults(&mut self) {
        self.pen_enabled = true;
        self.pen_hotkey = HotkeyConfig::ctrl("D1", false);
        self.laser_enabled = true;
        self.laser_hotkey = HotkeyConfig::ctrl("D2", false);
        self.rectangle_enabled = false;
        self.rectangle_hotkey = HotkeyConfig::default();
        self.ellipse_enabled = false;
        self.ellipse_hotkey = HotkeyConfig::default();
        self.arrow_enabled = false;
        self.arrow_hotkey = HotkeyConfig::default();
        self.line_enabled = false;
        self.line_hotkey = HotkeyConfig::default();
        self.spotlight_enabled = true;
        self.spotlight_hotkey = HotkeyConfig::ctrl("D3", false);
        self.arrow_bend_enabled = false;
        self.arrow_bend_hotkey = HotkeyConfig::default();
        self.cancel_enabled = true;
        self.cancel_hotkey = HotkeyConfig::new(Modifiers::empty(), "Escape");
        self.laser_width = 2.0;
        self.color_binds = Vec::new();
        self.defaults_revision = 5;
    }

    pub fn ensure_ink_colors(&mut self) {
        if self.ink_colors.is_empty() {
            let first = if self.pen_color.trim().is_empty() {
                DEFAULT_INK_COLORS[0]
            } else {
                &self.pen_color
            };
            self.ink_colors.push(first.to_string());
        }
        let start = self.ink_colors.len();
        for color in DEFAULT_INK_COLORS.iter().skip(start) {
            self.ink_colors.push((*color).to_string());
        }
        self.ink_colors.truncate(5);
        for (i, color) in self.ink_colors.iter_mut().enumerate() {
            if color.trim().is_empty() || color.len() < 7 {
                *color = DEFAULT_INK_COLORS[i].to_string();
            }
        }
    }
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.02
}

// ─── Root ─────────────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AppSettings {
    // General
    pub show_cursor: bool,
    pub auto_copy_link_after_upload: bool,
    pub auto_close_upload_window: bool,
    pub launch_at_startup: bool,

    // Hotkeys
    pub region_hotkey: HotkeyConfig,
    pub full_screen_hotkey: HotkeyConfig,
    pub full_screen_upload_hotkey: HotkeyConfig,
    pub collage_hotkey: HotkeyConfig,
    pub quick_translate_hotkey: HotkeyConfig,
    pub settings_revision: i32,

    // Output
    pub output_format: ImageFormat,
    pub quality: i32,
    pub save_directory: String,

    pub tool_styles: ToolStyleMemory,
    pub gif: GifSettings,
    pub video: VideoSettings,
    pub presenter: PresenterSettings,
    pub subtitle: SubtitleSettings,

    pub translate_source_language: String,
    pub translate_target_language: String,
    pub translate_native_language: String,
    pub translate_pair_language: String,
    pub recent_visual_languages: Vec<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            show_cursor: false,
            auto_copy_link_after_upload: true,
            auto_close_upload_window: false,
            launch_at_startup: true,
            region_hotkey: HotkeyConfig::alt_shift("S"),
            full_screen_hotkey: HotkeyConfig::default(),
            full_screen_upload_hotkey: HotkeyConfig::default(),
            collage_hotkey: HotkeyConfig::default(),
            quick_translate_hotkey: HotkeyConfig::alt_shift("T"),
            settings_revision: 0,
            output_format: ImageFormat::Png,
            quality: 92,
            save_directory: default_pictures_dir(),
            tool_styles: ToolStyleMemory::default(),
            gif: GifSettings::default(),
            video: VideoSettings::default(),
            presenter: PresenterSettings::default(),
            subtitle: SubtitleSettings::default(),
            translate_source_language: "auto".into(),
            translate_target_language: "tr".into(),
            translate_native_language: String::new(),
            translate_pair_language: String::new(),
            recent_visual_languages: Vec::new(),
        }
    }
}

impl AppSettings {
    /// Resets fields that no longer persist, clamps ranges and fills first-run
    /// language defaults. `ui_culture` is the two-letter UI language (e.g. "tr").
    pub fn normalize(&mut self, ui_culture: &str) {
        self.tool_styles.opacity = 1.0;

        self.gif.fps = self.gif.fps.clamp(1, 60);
        self.video.fps = self.video.fps.clamp(1, 60);

        self.presenter.normalize();
        self.subtitle.normalize();

        if self.translate_native_language.trim().is_empty() {
            self.translate_native_language = map_ui_culture(ui_culture);
        }
        if self.translate_pair_language.trim().is_empty() {
            self.translate_pair_language = default_pair(&self.translate_native_language);
        }

        let pair = self.translate_pair_language.trim().to_string();
        self.recent_visual_languages.retain(|c| {
            !c.trim().is_empty() && is_known(c) && !c.trim().eq_ignore_ascii_case(&pair)
        });

        if self.settings_revision < 2 {
            if !self.region_hotkey.is_valid() {
                self.region_hotkey = HotkeyConfig::alt_shift("S");
            }
            if !self.quick_translate_hotkey.is_valid() {
                self.quick_translate_hotkey = HotkeyConfig::alt_shift("T");
            }
            self.settings_revision = 2;
        }
    }

    /// Parses settings JSON. `None` means the text was not valid settings.
    pub fn deserialize(json: &str) -> Option<AppSettings> {
        serde_json::from_str(json).ok()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("settings serialize")
    }

    /// Loads settings from `path`. `is_first_run` is true when the file was
    /// missing or unreadable; a corrupt file is copied to `<path>.corrupt`.
    pub fn load(path: &Path) -> (AppSettings, bool) {
        if let Ok(json) = std::fs::read_to_string(path) {
            if let Some(mut loaded) = Self::deserialize(&json) {
                loaded.normalize(&current_ui_culture());
                return (loaded, false);
            }
            // Corrupt file: keep a copy for recovery, then fall back to defaults.
            let _ = std::fs::copy(path, corrupt_path(path));
        }

        let mut created = AppSettings::default();
        created.normalize(&current_ui_culture());
        (created, true)
    }

    /// Writes atomically: a temp file in the same directory, then a rename. Two
    /// writers can never leave a half-written file that fails to load.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let mut copy = self.clone();
        copy.normalize(&current_ui_culture());

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let json = copy.to_json();
        let tmp = tmp_path(path);
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

fn corrupt_path(path: &Path) -> std::path::PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".corrupt");
    std::path::PathBuf::from(s)
}

fn tmp_path(path: &Path) -> std::path::PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(format!(".{}.tmp", std::process::id()));
    std::path::PathBuf::from(s)
}

/// `%APPDATA%\ScreenForge`, the settings directory.
pub fn settings_directory() -> std::path::PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("ScreenForge")
}

pub fn settings_path() -> std::path::PathBuf {
    settings_directory().join("settings.json")
}

fn default_pictures_dir() -> String {
    match std::env::var_os("USERPROFILE") {
        Some(p) => std::path::PathBuf::from(p)
            .join("Pictures")
            .to_string_lossy()
            .into_owned(),
        None => String::new(),
    }
}

/// Best-effort UI culture. Windows exposes it through the environment; absent
/// that, fall back to "en" (the C# code defaults unknown cultures to "en" too).
fn current_ui_culture() -> String {
    std::env::var("LANG")
        .ok()
        .filter(|l| l.len() >= 2)
        .map(|l| l[..2].to_lowercase())
        .unwrap_or_else(|| "en".to_string())
}
