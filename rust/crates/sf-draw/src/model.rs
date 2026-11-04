//! Serializable shape model + a single render entry point.
//!
//! The interactive overlay collects `Shape`s as the user drags; the exporter draws
//! them through [`render`]. Keeping shapes as plain data means undo, export and
//! tests all work on the same representation.

use crate::canvas::{Canvas, Rgba};
use crate::shapes::{self, Stroke};

/// The annotation tools. Mirrors the C# `EditorTool` subset the rasterizer covers.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tool {
    Select,
    #[default]
    Rect,
    Ellipse,
    Arrow,
    Line,
    Pen,
    Highlight,
    Step,
    Blur,
    Text,
}

impl Tool {
    /// Cycles to the next tool when the user presses Tab.
    pub fn next(self) -> Tool {
        match self {
            Tool::Select => Tool::Rect,
            Tool::Rect => Tool::Ellipse,
            Tool::Ellipse => Tool::Arrow,
            Tool::Arrow => Tool::Line,
            Tool::Line => Tool::Pen,
            Tool::Pen => Tool::Highlight,
            Tool::Highlight => Tool::Step,
            Tool::Step => Tool::Blur,
            Tool::Blur => Tool::Text,
            Tool::Text => Tool::Select,
        }
    }

    /// A short label for the overlay's tool hint.
    pub fn name(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Rect => "Rect",
            Tool::Ellipse => "Ellipse",
            Tool::Arrow => "Arrow",
            Tool::Line => "Line",
            Tool::Pen => "Pen",
            Tool::Highlight => "Highlight",
            Tool::Step => "Step",
            Tool::Blur => "Blur",
            Tool::Text => "Text",
        }
    }
}

/// One drawn annotation, in the coordinate space it was drawn in.
#[derive(Clone, PartialEq, Debug)]
pub enum Shape {
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: Rgba,
    },
    Ellipse {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: Rgba,
    },
    Arrow {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        color: Rgba,
    },
    Line {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        color: Rgba,
    },
    Pen {
        points: Vec<(f32, f32)>,
        color: Rgba,
    },
    /// A thick translucent marker stroke.
    Highlight {
        points: Vec<(f32, f32)>,
        color: Rgba,
        width: f32,
    },
    Step {
        cx: f32,
        cy: f32,
        radius: f32,
        number: u32,
        color: Rgba,
    },
    /// A text label.
    Text {
        x: f32,
        y: f32,
        text: String,
        scale: f32,
        color: Rgba,
        ribbon: bool,
    },
    /// A blur/pixelate redaction over a rectangle.
    Blur {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        kind: crate::blur::MaskKind,
        strength: f32,
    },
}

impl Shape {
    /// Shifts the shape by `(dx, dy)` — used to translate screen-coord shapes into
    /// cropped-region coordinates.
    pub fn translate(&self, dx: f32, dy: f32) -> Shape {
        let mut s = self.clone();
        match &mut s {
            Shape::Rect { x, y, .. } | Shape::Ellipse { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
            Shape::Arrow { x0, y0, x1, y1, .. } | Shape::Line { x0, y0, x1, y1, .. } => {
                *x0 += dx;
                *y0 += dy;
                *x1 += dx;
                *y1 += dy;
            }
            Shape::Pen { points, .. } | Shape::Highlight { points, .. } => {
                for p in points.iter_mut() {
                    p.0 += dx;
                    p.1 += dy;
                }
            }
            Shape::Step { cx, cy, .. } => {
                *cx += dx;
                *cy += dy;
            }
            Shape::Blur { x, y, .. } | Shape::Text { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
        }
        s
    }
}

/// Line width used for every annotation, so a set of shapes looks consistent.
pub const LINE_WIDTH: f32 = 2.5;

/// Draws one shape with the standard stroke width.
pub fn render(canvas: &mut Canvas, shape: &Shape) {
    render_with(canvas, shape, LINE_WIDTH);
}

/// Draws one shape with an explicit stroke width. `Highlight`, `Step`, `Text`
/// and `Blur` carry their own sizing and ignore `width`.
pub fn render_with(canvas: &mut Canvas, shape: &Shape, width: f32) {
    match shape {
        Shape::Rect { x, y, w, h, color } => {
            shapes::draw_rect(canvas, *x, *y, *w, *h, Stroke::new(*color, width));
        }
        Shape::Ellipse { x, y, w, h, color } => {
            shapes::draw_ellipse(canvas, *x, *y, *w, *h, Stroke::new(*color, width));
        }
        Shape::Arrow { x0, y0, x1, y1, color } => {
            shapes::draw_arrow(canvas, *x0, *y0, *x1, *y1, Stroke::new(*color, width));
        }
        Shape::Line { x0, y0, x1, y1, color } => {
            shapes::draw_stroke(canvas, *x0, *y0, *x1, *y1, Stroke::new(*color, width));
        }
        Shape::Pen { points, color } => {
            shapes::draw_stroke_path(canvas, points, Stroke::new(*color, width));
        }
        Shape::Highlight { points, color, width } => {
            crate::highlight::draw_highlight(canvas, points, *color, *width);
        }
        Shape::Step {
            cx,
            cy,
            radius,
            number,
            color,
        } => {
            shapes::draw_step(canvas, *cx, *cy, *radius, *number, *color, Rgba::new(255, 255, 255, 255));
        }
        Shape::Text { x, y, text, scale, color, ribbon } => {
            let bg = if *ribbon {
                Some(Rgba::new(0x1F, 0x24, 0x30, 0xCC))
            } else {
                None
            };
            crate::text::draw_text(canvas, *x, *y, text, *scale, *color, bg);
        }
        Shape::Blur { x, y, w, h, kind, strength } => {
            // Blur/pixelate mutate the underlying pixels, so they go through the
            // canvas buffer rather than drawing atop it.
            canvas.apply_mask(
                (*x as i32, *y as i32, *w as i32, *h as i32),
                *kind,
                *strength,
            );
        }
    }
}

/// Draws a list of shapes in order.
pub fn render_all(canvas: &mut Canvas, shapes: &[Shape]) {
    for shape in shapes {
        render(canvas, shape);
    }
}
