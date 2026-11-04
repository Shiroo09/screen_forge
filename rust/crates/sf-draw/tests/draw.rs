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
