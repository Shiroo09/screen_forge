//! Input-layer logic ported from `ScreenForge/Gif/Input` and `ScreenForge/Hotkeys`.
//!
//! Key labels, the reserved-shortcut table, mouse button flags and the input
//! overlay color/geometry decisions. Screen capture itself (GDI/DXGI) and the
//! low-level hooks stay with the app shell.

pub mod clipboard;
pub mod hotkey;
pub mod editor_window;
pub mod input_hook;
pub mod overlay_window;
pub mod tray;
pub mod selection;
pub mod key_labels;
pub mod screen;
pub mod keys;
pub mod overlay;
pub mod reserved_hotkeys;

pub use clipboard::copy_bgra_to_clipboard;
pub use keys::MouseButtons;
pub use screen::{capture_region, capture_virtual_screen, virtual_screen_bounds, Captured};
pub use overlay::{InputOverlayOptions, Rgba};
