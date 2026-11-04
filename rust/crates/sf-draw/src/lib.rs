//! Software rasterizer for screenshot annotation.
//!
//! The C# editor drew shapes with SkiaSharp onto a WPF surface. This replaces the
//! shape drawing with pure-Rust scanning onto a BGRA buffer, so annotation works
//! headless and the pixels are testable. Covers the common tools: line, arrow,
//! rectangle, ellipse, freehand pen, and a numbered step marker.
//!
//! Coverage is currently the shape fill/stroke core; the WPF text tool and the
//! blur/pixelate item still belong to the editor UI.

pub mod blur;
pub mod canvas;
pub mod model;
pub mod presenter;
pub mod shapes;
pub mod text;

pub use blur::{apply_mask, MaskKind};
pub use canvas::{Canvas, Rgba};
pub use model::{render, render_all, Shape, Tool, LINE_WIDTH};
pub use text::{draw_text, measure};
pub use shapes::{draw_arrow, draw_ellipse, draw_rect, draw_step, draw_stroke, Stroke};
