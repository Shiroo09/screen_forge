//! Annotation shapes must actually mark pixels, stay inside their geometry, and
//! leave untouched pixels alone.

use sf_draw::canvas::{Canvas, Rgba};
use sf_draw::shapes::{self, Stroke};

const RED: Rgba = Rgba::new(255, 0, 0, 255);
const WHITE: Rgba = Rgba::new(255, 255, 255, 255);
const BLUE: Rgba = Rgba::new(0, 0, 255, 255);

fn surface(w: i32, h: i32) -> Vec<u8> {
    // Mid-grey opaque background.
    let mut px = vec![0u8; (w * h * 4) as usize];
    for chunk in px.chunks_exact_mut(4) {
        chunk[0] = 128;
        chunk[1] = 128;
        chunk[2] = 128;
        chunk[3] = 255;
    }
    px
}

#[test]
fn horizontal_line_marks_its_row_only() {
    let mut px = surface(32, 32);
    {
        let mut c = Canvas::new(&mut px, 32, 32);
        shapes::draw_stroke(&mut c, 4.0, 16.0, 28.0, 16.0, Stroke::new(RED, 2.0));
    }

    let c = Canvas::new(&mut px, 32, 32);
    // On the line: red.
    let mid = c.get(16, 16).unwrap();
    assert!(mid.r > 200 && mid.g < 60, "line pixel {mid:?}");
    // Two rows away: untouched grey.
    let off = c.get(16, 20).unwrap();
    assert_eq!(off.r, 128);
}

#[test]
fn stroke_path_draws_a_pen_trail() {
    let mut px = surface(32, 32);
    {
        let mut c = Canvas::new(&mut px, 32, 32);
        shapes::draw_stroke_path(&mut c, &[(2.0, 2.0), (16.0, 16.0), (30.0, 2.0)], Stroke::new(BLUE, 2.0));
    }

    let c = Canvas::new(&mut px, 32, 32);
    assert!(c.get(2, 2).unwrap().b > 200, "start of trail");
    assert!(c.get(16, 16).unwrap().b > 200, "peak of trail");
    assert!(c.get(30, 2).unwrap().b > 200, "end of trail");
}

#[test]
fn rect_outline_is_hollow() {
    let mut px = surface(40, 40);
    {
        let mut c = Canvas::new(&mut px, 40, 40);
        shapes::draw_rect(&mut c, 8.0, 8.0, 24.0, 24.0, Stroke::new(RED, 2.0));
    }
    let c = Canvas::new(&mut px, 40, 40);

    // A corner of the outline is red.
    assert!(c.get(8, 8).unwrap().r > 180);
    // The centre is untouched (outline only).
    let centre = c.get(20, 20).unwrap();
    assert_eq!((centre.r, centre.g, centre.b), (128, 128, 128));
}

#[test]
fn rect_with_fill_paints_the_interior() {
    let mut px = surface(40, 40);
    {
        let mut c = Canvas::new(&mut px, 40, 40);
        shapes::draw_rect(
            &mut c,
            8.0,
            8.0,
            24.0,
            24.0,
            Stroke::new(RED, 2.0).with_fill(BLUE),
        );
    }
    let c = Canvas::new(&mut px, 40, 40);
    let centre = c.get(20, 20).unwrap();
    assert!(centre.b > 200, "fill should be blue, got {centre:?}");
}

#[test]
fn ellipse_marks_edge_not_corner() {
    let mut px = surface(40, 40);
    {
        let mut c = Canvas::new(&mut px, 40, 40);
        shapes::draw_ellipse(&mut c, 4.0, 4.0, 32.0, 32.0, Stroke::new(RED, 2.0));
    }
    let c = Canvas::new(&mut px, 40, 40);

    // Left-middle of the inscribed ellipse is on the outline.
    assert!(c.get(4, 20).unwrap().r > 150, "left edge of ellipse");
    // The top-left corner of the bounding box is outside the ellipse.
    let corner = c.get(4, 4).unwrap();
    assert_eq!((corner.r, corner.g, corner.b), (128, 128, 128), "corner must be untouched");
}

#[test]
fn arrow_has_a_shaft_and_wider_head() {
    let mut px = surface(64, 64);
    {
        let mut c = Canvas::new(&mut px, 64, 64);
        shapes::draw_arrow(&mut c, 10.0, 32.0, 50.0, 32.0, Stroke::new(WHITE, 3.0));
    }

    // Count white pixels per column: the head columns should have more than the shaft.
    let c = Canvas::new(&mut px, 64, 64);
    let mut column_white = [0i32; 64];
    for x in 0..64 {
        for y in 0..64 {
            if c.get(x, y).unwrap().r > 200 {
                column_white[x as usize] += 1;
            }
        }
    }

    let shaft = column_white[20];
    let head = column_white[46];
    assert!(shaft >= 2, "shaft should be a couple of pixels tall, got {shaft}");
    assert!(head > shaft, "head ({head}) should be wider than shaft ({shaft})");
}

#[test]
fn step_draws_badge_and_digit() {
    let mut px = surface(64, 64);
    {
        let mut c = Canvas::new(&mut px, 64, 64);
        shapes::draw_step(&mut c, 32.0, 32.0, 16.0, 3, RED, WHITE);
    }
    let c = Canvas::new(&mut px, 64, 64);

    // Centre pixel: badge is red, but the digit "3" has segments through the middle
    // row, so some white must be present near the centre band.
    let mut white = 0;
    for x in 24..40 {
        if c.get(x, 32).unwrap().r > 200 && c.get(x, 32).unwrap().g > 200 {
            white += 1;
        }
    }
    assert!(white > 0, "digit segments should be visible inside the badge");

    // Outside the badge: untouched.
    assert_eq!(c.get(2, 2).unwrap().r, 128);
}

#[test]
fn outside_bounds_is_silently_ignored() {
    let mut px = surface(8, 8);
    let mut c = Canvas::new(&mut px, 8, 8);
    // These must not panic.
    shapes::draw_stroke(&mut c, -100.0, -100.0, -50.0, -50.0, Stroke::new(RED, 4.0));
    shapes::draw_rect(&mut c, 100.0, 100.0, 10.0, 10.0, Stroke::new(RED, 2.0));
    assert!(c.get(4, 4).is_some());
}

// ─── Shape model ──────────────────────────────────────────────────────────────

#[test]
fn tool_cycles_through_all_tools() {
    use sf_draw::model::Tool;
    let mut t = Tool::Rect;
    let mut seen = vec![t];
    for _ in 0..8 {
        t = t.next();
        seen.push(t);
    }
    assert_eq!(t, Tool::Rect, "must return to start after 8 steps");
    // All seven tools appear exactly once.
    let mut names: Vec<_> = seen.iter().map(|t| t.name()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 8);
}

#[test]
fn shape_translate_shifts_every_kind() {
    use sf_draw::model::Shape;
    let shapes = vec![
        Shape::Rect { x: 10.0, y: 20.0, w: 5.0, h: 5.0, color: RED },
        Shape::Arrow { x0: 0.0, y0: 0.0, x1: 10.0, y1: 10.0, color: RED },
        Shape::Pen { points: vec![(1.0, 1.0), (2.0, 2.0)], color: RED },
        Shape::Step { cx: 3.0, cy: 4.0, radius: 5.0, number: 1, color: RED },
    ];

    for s in &shapes {
        let t = s.translate(-10.0, -20.0);
        // Round-trip back to the original.
        assert_eq!(&t.translate(10.0, 20.0), s);
    }
}

#[test]
fn render_all_draws_in_order_on_capture() {
    use sf_draw::model::{render_all, Shape};

    let mut px = surface(64, 64);
    let shapes = vec![
        Shape::Rect { x: 8.0, y: 8.0, w: 40.0, h: 40.0, color: RED },
        Shape::Step { cx: 32.0, cy: 32.0, radius: 10.0, number: 1, color: BLUE },
    ];
    {
        let mut c = Canvas::new(&mut px, 64, 64);
        render_all(&mut c, &shapes);
    }

    let c = Canvas::new(&mut px, 64, 64);
    // Step (drawn last) paints its badge over the rect centre.
    assert!(c.get(32, 32).unwrap().b > 150, "badge should cover the centre");
    // The rect edge is still visible outside the badge.
    assert!(c.get(8, 28).unwrap().r > 150, "rect edge visible");
}

// ─── Blur / pixelate ──────────────────────────────────────────────────────────

/// A buffer with a hard black/white edge, to see the mask soften or block it.
fn edge_surface(w: i32, h: i32) -> Vec<u8> {
    let mut px = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let v = if x < w / 2 { 0 } else { 255 };
            px[i] = v;
            px[i + 1] = v;
            px[i + 2] = v;
            px[i + 3] = 255;
        }
    }
    px
}

#[test]
fn blur_softens_an_edge() {
    use sf_draw::blur::{apply_mask, MaskKind};
    let mut px = edge_surface(32, 16);
    // Blur a band straddling the edge.
    apply_mask(&mut px, 32, 16, (10, 0, 12, 16), MaskKind::Blur, 3.0);

    let c = Canvas::new(&mut px, 32, 16);
    // Right at the old edge, the pixel is now a mid tone, not pure black/white.
    let mid = c.get(16, 8).unwrap();
    assert!(mid.r > 40 && mid.r < 215, "edge should soften, got {mid:?}");
    // Far from the blurred rect, the original edge is untouched.
    let outside = c.get(0, 8).unwrap();
    assert_eq!(outside.r, 0);
}

#[test]
fn pixelate_blocks_an_edge() {
    use sf_draw::blur::{apply_mask, MaskKind};
    let mut px = edge_surface(32, 16);
    apply_mask(&mut px, 32, 16, (0, 0, 32, 16), MaskKind::Pixelate, 8.0);

    let c = Canvas::new(&mut px, 32, 16);
    // Every pixel in an 8x8 cell shares one colour.
    let a = c.get(0, 0).unwrap();
    let b = c.get(7, 7).unwrap();
    assert_eq!((a.r, a.g, a.b), (b.r, b.g, b.b), "cell must be uniform");
    // And neighbouring cells across the edge differ.
    let left = c.get(0, 0).unwrap();
    let right = c.get(24, 0).unwrap();
    assert!(right.r > left.r, "the two sides of the edge differ");
}

#[test]
fn mask_clamps_out_of_bounds_rect() {
    use sf_draw::blur::{apply_mask, MaskKind};
    let mut px = edge_surface(8, 8);
    // Must not panic or read out of range.
    apply_mask(&mut px, 8, 8, (-10, -10, 100, 100), MaskKind::Blur, 2.0);
    apply_mask(&mut px, 8, 8, (5, 5, 100, 100), MaskKind::Pixelate, 3.0);
}

#[test]
fn blur_shape_via_model_softens_pixels() {
    use sf_draw::model::{render, Shape};
    let mut px = edge_surface(32, 16);
    {
        let mut c = Canvas::new(&mut px, 32, 16);
        render(
            &mut c,
            &Shape::Blur {
                x: 10.0,
                y: 0.0,
                w: 12.0,
                h: 16.0,
                kind: sf_draw::blur::MaskKind::Blur,
                strength: 3.0,
            },
        );
    }
    let c = Canvas::new(&mut px, 32, 16);
    let mid = c.get(16, 8).unwrap();
    assert!(mid.r > 40 && mid.r < 215, "model blur should soften the edge");
}

// ─── Text ─────────────────────────────────────────────────────────────────────

#[test]
fn text_glyph_marks_pixels() {
    use sf_draw::text::{draw_text, measure};
    let mut px = surface(64, 32);

    let (w, h) = measure("HI", 2.0);
    assert!(w > 0.0 && h > 0.0);
    {
        let mut c = Canvas::new(&mut px, 64, 32);
        draw_text(&mut c, 4.0, 4.0, "HI", 2.0, WHITE, None);
    }

    let c = Canvas::new(&mut px, 64, 32);
    // Count non-background pixels in the text band.
    let mut marked = 0;
    for y in 4..30 {
        for x in 4..60 {
            if c.get(x, y).unwrap().r > 200 {
                marked += 1;
            }
        }
    }
    assert!(marked > 20, "text should mark many pixels, got {marked}");
}

#[test]
fn text_measure_scales_with_size() {
    use sf_draw::text::measure;
    let (w1, h1) = measure("ABC", 1.0);
    let (w2, h2) = measure("ABC", 2.0);
    assert!((w2 - w1 * 2.0).abs() < 0.1);
    assert!((h2 - h1 * 2.0).abs() < 0.1);
}

#[test]
fn text_ribbon_draws_background() {
    use sf_draw::model::{render, Shape};
    let mut px = surface(64, 32);
    {
        let mut c = Canvas::new(&mut px, 64, 32);
        render(
            &mut c,
            &Shape::Text {
                x: 8.0,
                y: 8.0,
                text: "OK".into(),
                scale: 2.0,
                color: WHITE,
                ribbon: true,
            },
        );
    }
    let c = Canvas::new(&mut px, 64, 32);
    // The ribbon is 0xCC-alpha over the grey backdrop: the result is darker than
    // grey and blue-tinted, not the raw ribbon colour.
    let pad = c.get(6, 6).unwrap();
    assert!(pad.r < 100 && pad.b >= pad.r, "ribbon should darken into a cool tone, got {pad:?}");
}

#[test]
fn text_tool_and_shape_translate() {
    use sf_draw::model::Shape;
    let s = Shape::Text {
        x: 10.0,
        y: 10.0,
        text: "X".into(),
        scale: 1.0,
        color: WHITE,
        ribbon: false,
    };
    assert_eq!(s.translate(-5.0, -5.0).translate(5.0, 5.0), s);
}

