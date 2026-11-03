//! Arrow bend control point. Port of `ScreenForge/Presenter/ArrowBend.cs`.
//!
//! Given a start and end point, picks a control point for a bowed quadratic arrow,
//! preferring the bend that stays inside the screen.

use sf_edit::view_transform::Point;

pub const BULGE: f32 = 0.22;

/// The control point for a bowed arrow from `start` to `end`, kept inside `screen`
/// (an inflated rectangle). `flip` picks the opposite side.
pub fn control_point(start: Point, end: Point, screen: (f32, f32, f32, f32), flip: bool) -> Point {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let len = (dx * dx + dy * dy).sqrt();
    let mid = Point::new((start.x + end.x) / 2.0, (start.y + end.y) / 2.0);
    if len < 4.0 {
        return mid;
    }

    let px = -dy / len;
    let py = dx / len;
    let offset = len * BULGE;
    let a = Point::new(mid.x + px * offset, mid.y + py * offset);
    let b = Point::new(mid.x - px * offset, mid.y - py * offset);

    let preferred = if (a.y - b.y).abs() >= 1.5 {
        if a.y <= b.y {
            a
        } else {
            b
        }
    } else if a.x >= b.x {
        a
    } else {
        b
    };

    let other = if preferred == a { b } else { a };

    let safe = inflate(screen, 24.0);
    let mut pick = preferred;
    if !contains(safe, preferred) && contains(safe, other) {
        pick = other;
    }

    if flip {
        opposite(pick, preferred, other)
    } else {
        pick
    }
}

/// Rectangle as `(left, top, right, bottom)`, the stand-in for `SKRect`.
fn inflate(rect: (f32, f32, f32, f32), pad: f32) -> (f32, f32, f32, f32) {
    (rect.0 + pad, rect.1 + pad, rect.2 - pad, rect.3 - pad)
}

fn contains(rect: (f32, f32, f32, f32), p: Point) -> bool {
    p.x >= rect.0 && p.x < rect.2 && p.y >= rect.1 && p.y < rect.3
}

fn opposite(pick: Point, a: Point, b: Point) -> Point {
    if pick == a {
        b
    } else {
        a
    }
}
