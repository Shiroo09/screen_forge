//! Editor logic ported from `ScreenForge/Gif/Editing` and `ScreenForge/Editor`.
//!
//! Covers the frame/document model, the pure frame and image transforms, shared
//! undo ordering and the canvas view transform. Scene rendering (SkiaSharp) and
//! the WPF canvas host stay with the UI layer.

pub mod frame;
pub mod frame_ops;
pub mod history;
pub mod image_ops;
pub mod view_transform;

pub use frame::{EditorFrame, FrameInput, MouseButtons};
pub use frame_ops::{
    adjust_delay, duplicate, move_left, move_right, reduce, remove, remove_after, remove_before,
    remove_duplicates, reverse, scale_delay, set_delay, set_fps, similarity_of, smooth_loop, trim,
    DelayMergeMode, DuplicateRemoval,
};
pub use history::{EditHistory, EditScope};
pub use image_ops::{clamp_rect, crop, resize, rotate, screen_rect_to_source, size_after_rotate, Rect, RotateDirection};
pub use view_transform::{compute, scene_to_wpf, wpf_to_scene, LayoutMode, Point};
