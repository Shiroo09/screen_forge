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

/// Installs the design tokens once at startup.
pub fn install(ctx: &Context) {
    install_fonts(ctx);
    ctx.all_styles_mut(style_tokens);
}

/// Uses the same font stack as the C# app (`Segoe UI Variable Text, Segoe UI`).
/// egui ships Ubuntu-Light, which reads noticeably wider; Segoe keeps the port
/// looking like the original.
fn install_fonts(ctx: &Context) {
    let mut fonts = egui::FontDefinitions::default();
    // Windows ships Segoe UI; fall back to the bundled font if it is missing.
    const SEGOE: &str = r"C:\Windows\Fonts\segoeui.ttf";
    if let Ok(bytes) = std::fs::read(SEGOE) {
        fonts
            .font_data
            .insert("segoe".to_owned(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts.families.entry(family).or_default().insert(0, "segoe".to_owned());
        }
    }
    ctx.set_fonts(fonts);
}

/// Draws a button that shows hover/pressed feedback in the given fill colours.
///
/// `egui::Button::fill` overrides all state visuals, so a hand-coloured button
/// would never react to the pointer. This instead swaps the per-state fills on a
/// cloned style, so the button keeps whatever size/shape `build` gives it while
/// gaining the Theme.xaml default / hover / pressed colours.
pub fn stateful_button(
    ui: &mut egui::Ui,
    base: Color32,
    hover: Color32,
    pressed: Color32,
    build: impl FnOnce() -> egui::Button<'static>,
) -> egui::Response {
    let mut style = (**ui.style()).clone();
    style.visuals.widgets.inactive.weak_bg_fill = base;
    style.visuals.widgets.inactive.bg_fill = base;
    style.visuals.widgets.hovered.weak_bg_fill = hover;
    style.visuals.widgets.hovered.bg_fill = hover;
    style.visuals.widgets.active.weak_bg_fill = pressed;
    style.visuals.widgets.active.bg_fill = pressed;
    ui.scope(|ui| {
        ui.set_style(style);
        ui.add(build())
    })
    .inner
}

/// A primary (accent) button with the original hover/pressed ramp.
pub fn accent_button(
    ui: &mut egui::Ui,
    build: impl FnOnce() -> egui::Button<'static>,
) -> egui::Response {
    stateful_button(ui, ACCENT, ACCENT_HOVER, ACCENT_PRESSED, build)
}

/// A secondary (surface-alt) button with the original hover/pressed ramp.
pub fn surface_button(
    ui: &mut egui::Ui,
    build: impl FnOnce() -> egui::Button<'static>,
) -> egui::Response {
    stateful_button(ui, SURFACE_HI, SURFACE_HOVER, SURFACE_HOVER, build)
}

/// Applies the design tokens to one style (called for the active theme).
fn style_tokens(style: &mut egui::Style) {
    style.visuals.dark_mode = true;
    style.visuals.panel_fill = BG;
    style.visuals.window_fill = SURFACE;
    // Text fields and combos use SurfaceAlt (#272D3B), matching EditorTextBox.
    style.visuals.extreme_bg_color = SURFACE_HI;
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.widgets.noninteractive.bg_fill = SURFACE;
    style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, BORDER);
    style.visuals.widgets.inactive.bg_fill = SURFACE_HI;
    style.visuals.widgets.inactive.weak_bg_fill = SURFACE_HI;
    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, BORDER);
    style.visuals.widgets.hovered.bg_fill = SURFACE_HOVER;
    style.visuals.widgets.hovered.weak_bg_fill = SURFACE_HOVER;
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, BORDER);
    style.visuals.widgets.active.bg_fill = ACCENT_PRESSED;
    style.visuals.selection.bg_fill = ACCENT;
    style.visuals.hyperlink_color = ACCENT;
    style.visuals.error_fg_color = DANGER;
    style.visuals.warn_fg_color = DANGER;
    // 6px corners everywhere, as in the XAML templates (was 8).
    style.visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);
    style.visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);
    style.visuals.widgets.active.corner_radius = egui::CornerRadius::same(6);
    style.visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(6);
    style.visuals.window_corner_radius = egui::CornerRadius::same(8);
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(14.0, 8.0);
    style.spacing.window_margin = egui::Margin::same(16);
}
