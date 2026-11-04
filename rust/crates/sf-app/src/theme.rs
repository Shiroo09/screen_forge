//! Visual design tokens and egui style installation.
use egui::{Color32, Context};

pub const BG: Color32 = Color32::from_rgb(0x12, 0x14, 0x1A);
pub const SURFACE: Color32 = Color32::from_rgb(0x1B, 0x1E, 0x27);
pub const SURFACE_HI: Color32 = Color32::from_rgb(0x24, 0x28, 0x33);
pub const ACCENT: Color32 = Color32::from_rgb(0xEA, 0x6F, 0x12);
pub const TEXT: Color32 = Color32::from_rgb(0xE6, 0xE8, 0xEE);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x9A, 0xA1, 0xB0);
pub const BORDER: Color32 = Color32::from_rgb(0x2E, 0x33, 0x40);

/// Installs the design tokens once at startup.
pub fn install(ctx: &Context) {
    ctx.all_styles_mut(style_tokens);
}

/// Applies the design tokens to one style (called for the active theme).
fn style_tokens(style: &mut egui::Style) {
    style.visuals.dark_mode = true;
    style.visuals.panel_fill = BG;
    style.visuals.window_fill = SURFACE;
    style.visuals.extreme_bg_color = Color32::from_rgb(0x0D, 0x0F, 0x14);
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.widgets.noninteractive.bg_fill = SURFACE;
    style.visuals.widgets.inactive.bg_fill = SURFACE_HI;
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(0x33, 0x39, 0x47);
    style.visuals.widgets.active.bg_fill = ACCENT;
    style.visuals.selection.bg_fill = ACCENT;
    style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, BORDER);
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(14.0, 8.0);
    style.spacing.window_margin = egui::Margin::same(16);
}
