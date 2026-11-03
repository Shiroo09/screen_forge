//! Input overlay drawn onto captured frames before encoding.
//! Port of the pure parts of `ScreenForge/Gif/Input/InputOverlayRenderer.cs`.
//!
//! The C# renderer used GDI+ for the highlight circle and the key badge text. The
//! circle is drawn here in software (alpha-blended), so a captured frame looks
//! the same. Badge *text* needs a font shaper and stays with the UI toolkits; the
//! badge box placement is computed here so the app only has to draw glyphs.

use crate::keys::MouseButtons;

/// ARGB color, the stand-in for `System.Drawing.Color`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rgba {
    pub a: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgba {
    pub const fn from_argb(a: u8, r: u8, g: u8, b: u8) -> Self {
        Self { a, r, g, b }
    }
}

/// How the input overlay should look. Defaults match ScreenToGif.
#[derive(Clone, Debug)]
pub struct InputOverlayOptions {
    pub highlight_clicks: bool,
    pub highlight_cursor: bool,
    pub show_keys: bool,
    pub radius: f64,
    pub cursor_highlight_color: Rgba,
    pub left_click_color: Rgba,
    pub right_click_color: Rgba,
    pub middle_click_color: Rgba,
    pub first_extra_click_color: Rgba,
    pub second_extra_click_color: Rgba,
    pub key_font_size: f32,
}

impl Default for InputOverlayOptions {
    fn default() -> Self {
        Self {
            highlight_clicks: true,
            highlight_cursor: false,
            show_keys: true,
            radius: 12.0,
            cursor_highlight_color: Rgba::from_argb(120, 255, 255, 0),
            left_click_color: Rgba::from_argb(120, 255, 255, 0),
            right_click_color: Rgba::from_argb(120, 255, 0, 0),
            middle_click_color: Rgba::from_argb(120, 0, 255, 255),
            first_extra_click_color: Rgba::from_argb(120, 255, 0, 128),
            second_extra_click_color: Rgba::from_argb(120, 255, 128, 0),
            key_font_size: 14.0,
        }
    }
}

impl InputOverlayOptions {
    pub fn has_work(&self) -> bool {
        self.highlight_clicks || self.highlight_cursor || self.show_keys
    }

    /// The highlight color for the held button, or the cursor color if none is held.
    pub fn color_for(&self, buttons: MouseButtons) -> Rgba {
        if buttons.contains(MouseButtons::LEFT) {
            self.left_click_color
        } else if buttons.contains(MouseButtons::RIGHT) {
            self.right_click_color
        } else if buttons.contains(MouseButtons::MIDDLE) {
            self.middle_click_color
        } else if buttons.contains(MouseButtons::EXTRA1) {
            self.first_extra_click_color
        } else if buttons.contains(MouseButtons::EXTRA2) {
            self.second_extra_click_color
        } else {
            self.cursor_highlight_color
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_for_picks_held_button() {
        let options = InputOverlayOptions::default();
        assert_eq!(options.color_for(MouseButtons(MouseButtons::LEFT)), options.left_click_color);
        assert_eq!(options.color_for(MouseButtons(MouseButtons::RIGHT)), options.right_click_color);
        assert_eq!(options.color_for(MouseButtons(MouseButtons::MIDDLE)), options.middle_click_color);
        assert_eq!(options.color_for(MouseButtons::default()), options.cursor_highlight_color);
    }
}
