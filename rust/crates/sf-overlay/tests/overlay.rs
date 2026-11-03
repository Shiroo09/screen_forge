//! Mirrors `ScreenForge.Tests/SubtitleMathTests.cs` and pins `ArrowBend` geometry.

use sf_edit::view_transform::Point;
use sf_overlay::arrow::control_point;
use sf_overlay::subtitle::{
    changed_samples, contains, default_box, fingerprint, nearly_same, normalize_ocr, ocr_size,
    push_outside, scale_bgra, target_language, SAMPLE_COUNT,
};

// ─── Subtitle math ────────────────────────────────────────────────────────────

#[test]
fn target_language_empty_uses_turkish() {
    assert_eq!(target_language(None), "tr");
    assert_eq!(target_language(Some("  ")), "tr");
    assert_eq!(target_language(Some(" en ")), "en");
}

#[test]
fn normalize_ocr_collapses_blank_lines() {
    assert_eq!(
        normalize_ocr(Some("  Merhaba  \r\n\n Dünya ")).as_deref(),
        Some("Merhaba\nDünya")
    );
    assert_eq!(normalize_ocr(Some("   \n")), None);
}

#[test]
fn fingerprint_changes_when_sampled_pixel_changes() {
    let a = vec![0u8; 16 * 8 * 4];
    let mut b = a.clone();
    b[0] = 255;

    assert_ne!(fingerprint(&a, 16, 8), 0);
    assert_ne!(fingerprint(&a, 16, 8), fingerprint(&b, 16, 8));
}

#[test]
fn default_box_moves_above_source_near_bottom() {
    let screen = (0, 0, 1920, 1080);
    let source = (400, 1000, 600, 60);
    let b = default_box(source, screen);

    assert!(b.1 + b.3 <= source.1, "box bottom {} must be above source top {}", b.1 + b.3, source.1);
    assert!(b.2 >= 280, "width {}", b.2);
}

#[test]
fn nearly_same_ignores_one_character_jitter() {
    assert!(nearly_same("Hello world", "Hello wor1d"));
    assert!(!nearly_same("Hello world", "Tamamen baska"));
}

#[test]
fn changed_samples_skips_tiny_noise() {
    let a = vec![0u8; SAMPLE_COUNT];
    let mut b = a.clone();
    b[0] = 10;
    assert_eq!(changed_samples(&a, &b), 0);

    b[3] = 80;
    assert_eq!(changed_samples(&a, &b), 1);
}

#[test]
fn push_outside_slides_off_blocked_edge() {
    let blocked = (100, 100, 200, 40);
    let overlapping = (80, 90, 50, 30);
    let cleared = push_outside(overlapping, blocked);
    assert!(!(cleared.0 < blocked.0 + blocked.2
        && cleared.0 + cleared.2 > blocked.0
        && cleared.1 < blocked.1 + blocked.3
        && cleared.1 + cleared.3 > blocked.1));

    let far = (10, 10, 40, 20);
    assert_eq!(push_outside(far, blocked), far);
}

#[test]
fn ocr_size_shrinks_wide_regions() {
    let (w, h) = ocr_size(2560, 200);
    assert!(w <= 1280);
    assert!(h <= 240);
    assert_eq!(ocr_size(800, 80), (800, 80));
}

#[test]
fn scale_bgra_nearest_neighbor_halves() {
    // 2x2 -> 1x1: takes the top-left pixel.
    let src = vec![1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255];
    let out = scale_bgra(&src, 2, 2, 1, 1);
    assert_eq!(out, vec![1, 2, 3, 255]);
}

#[test]
fn contains_is_half_open() {
    assert!(contains(5, 5, 0, 0, 10, 10));
    assert!(!contains(10, 5, 0, 0, 10, 10)); // right edge excluded
    assert!(!contains(5, 5, 0, 0, 0, 10)); // zero width
}

// ─── Arrow bend ───────────────────────────────────────────────────────────────

#[test]
fn short_arrow_returns_midpoint() {
    let screen = (0.0, 0.0, 1000.0, 1000.0);
    let p = control_point(Point::new(10.0, 10.0), Point::new(12.0, 10.0), screen, false);
    assert_eq!(p, Point::new(11.0, 10.0)); // len < 4
}

#[test]
fn long_arrow_bends_off_the_straight_line() {
    let screen = (0.0, 0.0, 1000.0, 1000.0);
    let start = Point::new(100.0, 500.0);
    let end = Point::new(900.0, 500.0);
    let p = control_point(start, end, screen, false);

    let mid = Point::new(500.0, 500.0);
    // Exactly horizontal: the bend must leave the y = 500 line.
    assert!((p.y - mid.y).abs() > 1.0, "control {p:?} should bend off y=500");
}

#[test]
fn flip_picks_the_opposite_side() {
    let screen = (0.0, 0.0, 1000.0, 1000.0);
    let start = Point::new(100.0, 500.0);
    let end = Point::new(900.0, 500.0);

    let normal = control_point(start, end, screen, false);
    let flipped = control_point(start, end, screen, true);

    assert_ne!(normal, flipped);
    // Opposite sides of the straight line.
    assert!((normal.y - 500.0).signum() != (flipped.y - 500.0).signum());
}
