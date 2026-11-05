//! The full-screen capture overlay: shows the frozen screenshot dimmed, tracks a
//! mouse drag, and returns the selected rectangle. Port of the region-selection
//! half of `ScreenForge/Windows/CaptureOverlayWindow.xaml.cs`.
//!
//! The frozen screenshot is what makes selection instant — the screen is captured
//! once when the overlay opens and never re-read, so nothing flickers.

use crate::selection::SelectionRect;

/// Composites the live mouse cursor onto a captured frame. `origin_x`/`origin_y`
/// are the buffer's top-left corner in virtual-screen coordinates, so the cursor
/// lands at the right spot even on a secondary monitor with negative origin.
/// Skips silently when the cursor is hidden or outside the frame.
#[cfg(windows)]
pub fn draw_cursor(shot: &mut crate::screen::Captured, origin_x: i32, origin_y: i32) -> bool {
    crate::screen::composite_cursor(shot, origin_x, origin_y)
}

#[cfg(not(windows))]
pub fn draw_cursor(_shot: &mut crate::screen::Captured, _origin_x: i32, _origin_y: i32) -> bool {
    false
}

/// Runs the region picker and returns the chosen rectangle in virtual-screen
/// coordinates, or `None` if the user cancelled (Esc / right-click).
#[cfg(windows)]
pub fn pick_region() -> Option<SelectionRect> {
    // Krom (logo, mod çubuğu, karartma) tek yerde yaşasın diye zengin overlay'e
    // devredilir. Burada ikinci bir pencere tutmak, aynı özelliğin iki farklı
    // arayüzle görünmesine yol açıyordu: kısayoldan açılan menülü/karartmalı,
    // "Bölge seç" ve `sf-cli select`'ten açılan menüsüz/karartmasız.
    crate::editor_window::pick_region_only()
}

#[cfg(not(windows))]
pub fn pick_region() -> Option<SelectionRect> {
    None
}
