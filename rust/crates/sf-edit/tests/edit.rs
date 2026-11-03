//! Mirrors `ScreenForge.Tests/{EditorOperationsTests,CanvasViewTransformTests}.cs`.

use std::sync::Arc;

use sf_edit::frame::{EditorFrame, FrameInput, MouseButtons};
use sf_edit::frame_ops::{
    adjust_delay, duplicate, move_left, move_right, reduce, remove, remove_after, remove_before,
    remove_duplicates, reverse, scale_delay, set_delay, set_fps, similarity_of, smooth_loop, trim,
    DelayMergeMode, DuplicateRemoval,
};
use sf_edit::image_ops::{self, Rect, RotateDirection};
use sf_edit::view_transform::{self, LayoutMode, Point};

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn make_pixels(w: usize, h: usize, value: u8) -> Vec<u8> {
    let mut px = vec![0u8; w * h * 4];
    for chunk in px.chunks_exact_mut(4) {
        chunk[0] = value;
        chunk[1] = value;
        chunk[2] = value;
        chunk[3] = 255;
    }
    px
}

fn make_frame(w: usize, h: usize, value: u8, delay: i32) -> Arc<EditorFrame> {
    EditorFrame::new(make_pixels(w, h, value), delay, FrameInput::default())
}

fn make_frames(count: usize) -> Vec<Arc<EditorFrame>> {
    (0..count)
        .map(|i| make_frame(4, 4, (i * 20 + 10) as u8, 100))
        .collect()
}

fn make_frame_with(pixels: Vec<u8>, delay: i32) -> Arc<EditorFrame> {
    EditorFrame::new(pixels, delay, FrameInput::default())
}

// ─── Frame operations ─────────────────────────────────────────────────────────

#[test]
fn remove_adds_delay_to_previous_frame() {
    let frames = vec![
        make_frame(2, 2, 0x10, 100),
        make_frame(2, 2, 0x20, 200),
        make_frame(2, 2, 0x30, 300),
    ];

    let result = remove(&frames, &[1], DelayMergeMode::AddToPrevious);

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].delay, 300); // 100 + 200
    assert_eq!(result[1].delay, 300);
}

#[test]
fn remove_discard_mode_shortens_animation() {
    let frames = vec![make_frame(2, 2, 0x10, 100), make_frame(2, 2, 0x20, 200)];

    let result = remove(&frames, &[1], DelayMergeMode::Discard);

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].delay, 100);
}

#[test]
fn remove_keeps_at_least_one_frame() {
    let frames = make_frames(2);
    let result = remove(&frames, &[0, 1], DelayMergeMode::AddToPrevious);
    assert_eq!(result.len(), 2);
}

#[test]
fn remove_first_frame_carries_delay_forward() {
    let frames = vec![make_frame(2, 2, 0x10, 100), make_frame(2, 2, 0x20, 200)];

    let result = remove(&frames, &[0], DelayMergeMode::AddToPrevious);

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].delay, 300);
}

#[test]
fn remove_before_and_after_keep_anchor_frame() {
    let frames = make_frames(5);
    assert_eq!(remove_before(&frames, 2, DelayMergeMode::Discard).len(), 3);
    assert_eq!(remove_after(&frames, 2, DelayMergeMode::Discard).len(), 3);
}

#[test]
fn trim_keeps_inclusive_range() {
    let frames = make_frames(6);
    let result = trim(&frames, 1, 3);

    assert_eq!(result.len(), 3);
    assert!(Arc::ptr_eq(&frames[1], &result[0]));
    assert!(Arc::ptr_eq(&frames[3], &result[2]));
}

#[test]
fn duplicate_inserts_copy_after_each_selection() {
    let frames = make_frames(3);
    let result = duplicate(&frames, &[0, 2]);

    assert_eq!(result.len(), 5);
    assert!(Arc::ptr_eq(&frames[0], &result[0]));
    assert!(Arc::ptr_eq(&frames[0], &result[1]));
    assert!(Arc::ptr_eq(&frames[2], &result[4]));
}

#[test]
fn reverse_inverts_order() {
    let frames = make_frames(3);
    let result = reverse(&frames);
    assert!(Arc::ptr_eq(&frames[2], &result[0]));
    assert!(Arc::ptr_eq(&frames[0], &result[2]));
}

#[test]
fn move_left_and_right_swap_neighbours() {
    let frames = make_frames(3);

    let left = move_left(&frames, &[1]);
    assert!(Arc::ptr_eq(&frames[1], &left[0]));
    assert!(Arc::ptr_eq(&frames[0], &left[1]));

    let right = move_right(&frames, &[1]);
    assert!(Arc::ptr_eq(&frames[2], &right[1]));
    assert!(Arc::ptr_eq(&frames[1], &right[2]));
}

#[test]
fn move_left_ignores_first_frame() {
    let frames = make_frames(3);
    let result = move_left(&frames, &[0]);
    assert!(Arc::ptr_eq(&frames[0], &result[0]));
}

#[test]
fn set_delay_only_touches_selection() {
    let frames = vec![make_frame(2, 2, 0x10, 100), make_frame(2, 2, 0x20, 100)];
    let result = set_delay(&frames, &[1], 500);
    assert_eq!(result[0].delay, 100);
    assert_eq!(result[1].delay, 500);
}

#[test]
fn scale_delay_halves_duration() {
    let frames = vec![make_frame(2, 2, 0x10, 200)];
    let result = scale_delay(&frames, &[0], 50.0);
    assert_eq!(result[0].delay, 100);
}

#[test]
fn adjust_delay_clamps_to_minimum() {
    let frames = vec![make_frame(2, 2, 0x10, 20)];
    let result = adjust_delay(&frames, &[0], -100);
    assert_eq!(result[0].delay, 10); // GIF's meaningful lower bound
}

#[test]
fn set_fps_applies_uniform_delay() {
    let result = set_fps(&make_frames(3), 20);
    assert!(result.iter().all(|f| f.delay == 50));
}

#[test]
fn reduce_drops_frames_and_preserves_total_duration() {
    let frames: Vec<_> = (0..10)
        .map(|i| make_frame(2, 2, (i * 10) as u8, 100))
        .collect();
    let total: i32 = frames.iter().map(|f| f.delay).sum();

    let result = reduce(&frames, 2, 1, DelayMergeMode::Distribute);

    assert!(result.len() < frames.len());
    let result_total: i32 = result.iter().map(|f| f.delay).sum();
    assert_eq!(total, result_total);
}

#[test]
fn reduce_keeps_last_frame_pixels() {
    let frames: Vec<_> = (0..9)
        .map(|i| make_frame(2, 2, (i * 10) as u8, 100))
        .collect();

    let result = reduce(&frames, 1, 1, DelayMergeMode::Distribute);

    // The animation's ending is preserved; the delay may change via distribution.
    assert_eq!(result.last().unwrap().pixels(), frames.last().unwrap().pixels());
}

#[test]
fn remove_duplicates_collapses_identical_run() {
    let pixels = make_pixels(4, 4, 0x40);
    let frames = vec![
        make_frame_with(pixels.clone(), 100),
        make_frame_with(pixels.clone(), 100),
        make_frame_with(pixels.clone(), 100),
        make_frame(4, 4, 0x90, 100),
    ];

    let result = remove_duplicates(
        &frames,
        100.0,
        DuplicateRemoval::Last,
        DelayMergeMode::AddToPrevious,
        true,
    );

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].delay, 300); // three identical frames merged
}

#[test]
fn remove_duplicates_respects_similarity_threshold() {
    let base = make_pixels(4, 4, 0x40);
    let mut nearly = base.clone();
    // 2 of 16 pixels differ -> 87.5% similar.
    nearly[0..4].copy_from_slice(&[0xFF, 0xFF, 0xFF, 255]);
    nearly[4..8].copy_from_slice(&[0xFF, 0xFF, 0xFF, 255]);

    let frames = vec![
        make_frame_with(base, 100),
        make_frame_with(nearly, 100),
    ];

    let strict = remove_duplicates(&frames, 95.0, DuplicateRemoval::Last, DelayMergeMode::AddToPrevious, true);
    assert_eq!(strict.len(), 2);

    let loose = remove_duplicates(&frames, 80.0, DuplicateRemoval::Last, DelayMergeMode::AddToPrevious, true);
    assert_eq!(loose.len(), 1);
}

#[test]
fn remove_duplicates_keeps_frames_carrying_input() {
    let pixels = make_pixels(4, 4, 0x40);
    let with_click = EditorFrame::new(
        pixels.clone(),
        100,
        FrameInput {
            cursor_visible: true,
            buttons: MouseButtons(MouseButtons::LEFT),
            click_started: true,
            ..Default::default()
        },
    );

    let frames = vec![make_frame_with(pixels.clone(), 100), with_click];

    // Visually identical but carries a click -> must survive.
    let kept = remove_duplicates(&frames, 100.0, DuplicateRemoval::Last, DelayMergeMode::AddToPrevious, true);
    assert_eq!(kept.len(), 2);

    let dropped = remove_duplicates(&frames, 100.0, DuplicateRemoval::Last, DelayMergeMode::AddToPrevious, false);
    assert_eq!(dropped.len(), 1);
}

#[test]
fn smooth_loop_trims_tail_matching_first_frame() {
    let first = make_pixels(4, 4, 0x22);
    let frames = vec![
        make_frame_with(first.clone(), 100),
        make_frame(4, 4, 0x55, 100),
        make_frame(4, 4, 0x88, 100),
        make_frame_with(first, 100), // same as first -> redundant in the loop
    ];

    let result = smooth_loop(&frames, 99.0, 2);
    assert_eq!(result.len(), 3);
}

#[test]
fn smooth_loop_leaves_sequence_without_loop_point() {
    let frames = vec![
        make_frame(4, 4, 0x11, 100),
        make_frame(4, 4, 0x55, 100),
        make_frame(4, 4, 0x99, 100),
    ];
    assert_eq!(smooth_loop(&frames, 99.0, 2).len(), 3);
}

#[test]
fn similarity_reports_exact_match() {
    let a = make_pixels(4, 4, 0x30);
    let b = make_pixels(4, 4, 0x30);
    assert_eq!(similarity_of(&a, &b), 100.0);
}

// ─── Image operations ─────────────────────────────────────────────────────────

#[test]
fn crop_reduces_frame_to_rect() {
    let frames = vec![make_frame(8, 8, 0x30, 100)];
    let result = image_ops::crop(&frames, 8, 8, Rect::new(2, 2, 4, 4));
    assert_eq!(result[0].pixels().len(), 4 * 4 * 4);
}

#[test]
fn crop_clamps_rect_to_bounds() {
    let rect = image_ops::clamp_rect(Rect::new(6, 6, 10, 10), 8, 8);
    assert_eq!(rect, Rect::new(6, 6, 2, 2));
}

#[test]
fn rotate_right90_moves_top_left_to_top_right() {
    // 2x1: [A][B] -> right 90° -> 1x2 vertical: A on top, B below
    let mut pixels = vec![0u8; 8]; // 2x1 BGRA
    set_pixel(&mut pixels, 0, 10, 20, 30);
    set_pixel(&mut pixels, 1, 40, 50, 60);

    let rotated = image_ops::rotate_pixels(&pixels, 2, 1, RotateDirection::Right90);

    assert_eq!(rotated[0], 10); // A first
    assert_eq!(rotated[4], 40); // B second
}

#[test]
fn rotate_half180_reverses_pixels() {
    let mut pixels = vec![0u8; 8]; // 2x1 BGRA
    set_pixel(&mut pixels, 0, 10, 20, 30);
    set_pixel(&mut pixels, 1, 40, 50, 60);

    let rotated = image_ops::rotate_pixels(&pixels, 2, 1, RotateDirection::Half180);

    assert_eq!(rotated[0], 40);
    assert_eq!(rotated[4], 10);
}

#[test]
fn screen_rect_to_source_divides_by_zoom() {
    let rect = image_ops::screen_rect_to_source(20.0, 10.0, 100.0, 80.0, 0.5, 400, 300, 2);
    assert_eq!(rect, Some(Rect::new(40, 20, 200, 160)));
}

#[test]
fn screen_rect_to_source_is_identity_at_full_zoom() {
    let rect = image_ops::screen_rect_to_source(5.0, 7.0, 30.0, 40.0, 1.0, 100, 100, 2);
    assert_eq!(rect, Some(Rect::new(5, 7, 30, 40)));
}

#[test]
fn screen_rect_to_source_clamps_to_image_bounds() {
    let rect = image_ops::screen_rect_to_source(180.0, 0.0, 100.0, 50.0, 1.0, 200, 100, 2);
    assert_eq!(rect, Some(Rect::new(180, 0, 20, 50)));
}

#[test]
fn screen_rect_to_source_rejects_tiny_selection() {
    assert!(image_ops::screen_rect_to_source(0.0, 0.0, 1.0, 1.0, 1.0, 100, 100, 2).is_none());
    assert!(image_ops::screen_rect_to_source(0.0, 0.0, 0.0, 0.0, 1.0, 100, 100, 2).is_none());
}

#[test]
fn size_after_rotate_swaps_for_quarter_turns() {
    assert_eq!(image_ops::size_after_rotate(8, 6, RotateDirection::Right90), (6, 8));
    assert_eq!(image_ops::size_after_rotate(8, 6, RotateDirection::Left90), (6, 8));
    assert_eq!(image_ops::size_after_rotate(8, 6, RotateDirection::Half180), (8, 6));
}

#[test]
fn resize_produces_requested_dimensions() {
    let frames = vec![make_frame(16, 16, 0x50, 100)];
    let result = image_ops::resize(&frames, 16, 16, 8, 8);
    assert_eq!(result[0].pixels().len(), 8 * 8 * 4);
}

#[test]
fn resize_keeps_pixels_opaque() {
    let frames = vec![make_frame(8, 8, 0x50, 100)];
    let result = image_ops::resize(&frames, 8, 8, 4, 4);
    let pixels = result[0].pixels();
    for chunk in pixels.chunks_exact(4) {
        assert_eq!(chunk[3], 255);
    }
}

fn set_pixel(pixels: &mut [u8], index: usize, b: u8, g: u8, r: u8) {
    let offset = index * 4;
    pixels[offset] = b;
    pixels[offset + 1] = g;
    pixels[offset + 2] = r;
    pixels[offset + 3] = 255;
}

// ─── Canvas view transform ────────────────────────────────────────────────────

#[test]
fn wpf_to_scene_at_200dpi_maps_screen_center_to_scene_center() {
    // 1000 DIP canvas, 2000 px surface, scene 1000 — the midpoint must map to the scene's centre.
    let (scale, offset) = view_transform::compute(LayoutMode::OneToOne, 1000.0, 1000.0, 2000, 2000, 2.0);
    let p = view_transform::wpf_to_scene(500.0, 500.0, 1000.0, 1000.0, 2000, 2000, scale, offset);

    assert!((499.0..=501.0).contains(&p.x));
    assert!((499.0..=501.0).contains(&p.y));
}

#[test]
fn wpf_to_scene_when_surface_is_dip_sized_does_not_hit_scene_edge() {
    let (scale, offset) = view_transform::compute(LayoutMode::OneToOne, 1000.0, 1000.0, 1000, 1000, 2.0);
    let p = view_transform::wpf_to_scene(500.0, 500.0, 1000.0, 1000.0, 1000, 1000, scale, offset);

    assert!((499.0..=501.0).contains(&p.x));
    assert!((499.0..=501.0).contains(&p.y));
    assert!(p.x < 900.0, "screen centre must not stick to the scene edge");
}

#[test]
fn scene_to_wpf_inverts_wpf_to_scene() {
    let (scale, offset) = view_transform::compute(LayoutMode::OneToOne, 800.0, 600.0, 1600, 1200, 2.0);
    let scene = view_transform::wpf_to_scene(120.0, 80.0, 800.0, 600.0, 1600, 1200, scale, offset);
    let (x, y) = view_transform::scene_to_wpf(scene.x, scene.y, 800.0, 600.0, 1600, 1200, scale, offset);

    assert!((119.5..=120.5).contains(&x));
    assert!((79.5..=80.5).contains(&y));
}

#[test]
fn fit_layout_centers_small_scene() {
    let (scale, offset) =
        view_transform::compute(LayoutMode::Fit, 100.0, 50.0, 200, 200, 1.0);
    assert!(offset.x > 0.0 || offset.y > 0.0);

    let origin = view_transform::wpf_to_scene(offset.x as f64, offset.y as f64, 200.0, 200.0, 200, 200, scale, offset);
    assert!((-0.5..=0.5).contains(&origin.x));
    assert!((-0.5..=0.5).contains(&origin.y));
}

#[test]
fn compute_with_zero_scene_returns_identity() {
    let (scale, offset) = view_transform::compute(LayoutMode::Fit, 0.0, 0.0, 100, 100, 1.0);
    assert_eq!(scale, 1.0);
    assert_eq!(offset, Point::new(0.0, 0.0));
}
