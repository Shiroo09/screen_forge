//! Visual design tokens, matched one-to-one to the C# app's `Resources/Theme.xaml`
//! so the Rust build looks identical to the original.
use egui::{Color32, Context};

// Palette — values copied verbatim from Theme.xaml.
pub const ACCENT: Color32 = Color32::from_rgb(0xEA, 0x6F, 0x12);
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(0xF2, 0x8A, 0x3A);
pub const ACCENT_PRESSED: Color32 = Color32::from_rgb(0xD0, 0x62, 0x0F);
pub const SURFACE: Color32 = Color32::from_rgb(0x1F, 0x24, 0x30);
pub const SURFACE_HI: Color32 = Color32::from_rgb(0x27, 0x2D, 0x3B);
pub const SURFACE_HOVER: Color32 = Color32::from_rgb(0x32, 0x3A, 0x4C);
pub const BORDER: Color32 = Color32::from_rgb(0x3A, 0x42, 0x54);
pub const TEXT: Color32 = Color32::from_rgb(0xF2, 0xF4, 0xF8);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x9A, 0xA4, 0xB8);
pub const DANGER: Color32 = Color32::from_rgb(0xE5, 0x48, 0x4D);
/// Editor canvas / app background.
pub const BG: Color32 = Color32::from_rgb(0x12, 0x15, 0x1E);
/// Editor well (deeper than the canvas).
pub const WELL: Color32 = Color32::from_rgb(0x0E, 0x11, 0x18);
pub const CHROME: Color32 = Color32::from_rgb(0x17, 0x1B, 0x26);
pub const PANEL: Color32 = Color32::from_rgb(0x1C, 0x21, 0x30);

/// Installs the design tokens once at startup.
pub fn install(ctx: &Context) {
    ctx.all_styles_mut(style_tokens);
}

/// Applies the design tokens to one style (called for the active theme).
fn style_tokens(style: &mut egui::Style) {
    style.visuals.dark_mode = true;
    style.visuals.panel_fill = BG;
    style.visuals.window_fill = SURFACE;
    style.visuals.extreme_bg_color = WELL;
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.widgets.noninteractive.bg_fill = SURFACE;
    style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, BORDER);
    style.visuals.widgets.inactive.bg_fill = SURFACE_HI;
    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, BORDER);
    style.visuals.widgets.hovered.bg_fill = SURFACE_HOVER;
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, BORDER);
    style.visuals.widgets.active.bg_fill = ACCENT_PRESSED;
    style.visuals.selection.bg_fill = ACCENT;
    style.visuals.hyperlink_color = ACCENT;
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(14.0, 8.0);
    style.spacing.window_margin = egui::Margin::same(16);
}
