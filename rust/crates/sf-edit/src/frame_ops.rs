//! Pure transforms over a frame array. Port of `ScreenForge/Gif/Editing/FrameOperations.cs`.
//!
//! Every function returns a new list; the input is never modified.

use std::sync::Arc;

use crate::frame::EditorFrame;

/// What happens to a removed frame's delay.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum DelayMergeMode {
    /// Discard it; the animation shortens.
    Discard,
    /// Add it to the previous frame; total duration is preserved.
    #[default]
    AddToPrevious,
    /// Spread removed delays across the remaining frames; total preserved.
    Distribute,
}

/// Which frame of a duplicate pair is dropped.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum DuplicateRemoval {
    /// Drop the first of the pair.
    First,
    /// Drop the second of the pair.
    #[default]
    Last,
}

/// The longest delay one frame can carry (GIF field is 16-bit, 1/100 s).
const MAX_DELAY_MS: i32 = 65535 * 10;
const MIN_DELAY_MS: i32 = 10;

fn clamp_delay(delay: i32) -> i32 {
    delay.clamp(MIN_DELAY_MS, MAX_DELAY_MS)
}

// ─── Removal ──────────────────────────────────────────────────────────────────

/// Removes the given indices. Always leaves at least one frame.
pub fn remove(
    frames: &[Arc<EditorFrame>],
    indexes: &[usize],
    delay_mode: DelayMergeMode,
) -> Vec<Arc<EditorFrame>> {
    let to_remove: std::collections::HashSet<usize> = indexes
        .iter()
        .copied()
        .filter(|&i| i < frames.len())
        .collect();
    if to_remove.is_empty() || to_remove.len() >= frames.len() {
        return frames.to_vec();
    }

    let mut result: Vec<Arc<EditorFrame>> = Vec::with_capacity(frames.len() - to_remove.len());
    let mut carried = 0i32;

    for (i, frame) in frames.iter().enumerate() {
        if to_remove.contains(&i) {
            if delay_mode != DelayMergeMode::Discard {
                carried += frame.delay;
            }
            continue;
        }

        let mut frame = Arc::clone(frame);

        // Pass accumulated delay to the first suitable neighbour.
        if carried > 0 && delay_mode == DelayMergeMode::AddToPrevious {
            if let Some(last) = result.last() {
                let merged = last.with_delay(clamp_delay(last.delay + carried));
                *result.last_mut().unwrap() = merged;
            } else {
                // Leading frames were removed; no previous frame, so add to the next.
                frame = frame.with_delay(clamp_delay(frame.delay + carried));
            }
            carried = 0;
        }

        result.push(frame);
    }

    if carried > 0 && !result.is_empty() && delay_mode == DelayMergeMode::AddToPrevious {
        if let Some(last) = result.last() {
            let merged = last.with_delay(clamp_delay(last.delay + carried));
            *result.last_mut().unwrap() = merged;
        }
    }

    if delay_mode == DelayMergeMode::Distribute && carried > 0 && !result.is_empty() {
        distribute_delay(&mut result, carried);
    }

    result
}

/// Removes every frame before `index`.
pub fn remove_before(
    frames: &[Arc<EditorFrame>],
    index: usize,
    delay_mode: DelayMergeMode,
) -> Vec<Arc<EditorFrame>> {
    if index == 0 {
        frames.to_vec()
    } else {
        let range: Vec<usize> = (0..index).collect();
        remove(frames, &range, delay_mode)
    }
}

/// Removes every frame after `index`.
pub fn remove_after(
    frames: &[Arc<EditorFrame>],
    index: usize,
    delay_mode: DelayMergeMode,
) -> Vec<Arc<EditorFrame>> {
    if index + 1 >= frames.len() {
        frames.to_vec()
    } else {
        let range: Vec<usize> = (index + 1..frames.len()).collect();
        remove(frames, &range, delay_mode)
    }
}

/// Keeps the inclusive range `start..=end`, dropping everything else.
pub fn trim(frames: &[Arc<EditorFrame>], start: usize, end: usize) -> Vec<Arc<EditorFrame>> {
    if frames.is_empty() {
        return Vec::new();
    }
    let last = frames.len() - 1;
    let start = start.min(last);
    let end = end.clamp(start, last);
    frames[start..=end].to_vec()
}

// ─── Duplication and ordering ─────────────────────────────────────────────────

/// Copies the selected frames immediately after themselves.
pub fn duplicate(frames: &[Arc<EditorFrame>], indexes: &[usize]) -> Vec<Arc<EditorFrame>> {
    let to_copy: std::collections::HashSet<usize> = indexes
        .iter()
        .copied()
        .filter(|&i| i < frames.len())
        .collect();
    if to_copy.is_empty() {
        return frames.to_vec();
    }

    let mut result = Vec::with_capacity(frames.len() + to_copy.len());
    for (i, frame) in frames.iter().enumerate() {
        result.push(Arc::clone(frame));
        // Pixel buffers are shared — frames are immutable, so this is safe.
        if to_copy.contains(&i) {
            result.push(Arc::clone(frame));
        }
    }
    result
}

/// Reverses the frame order.
pub fn reverse(frames: &[Arc<EditorFrame>]) -> Vec<Arc<EditorFrame>> {
    let mut result = frames.to_vec();
    result.reverse();
    result
}

/// Moves the selected frames one position left.
pub fn move_left(frames: &[Arc<EditorFrame>], indexes: &[usize]) -> Vec<Arc<EditorFrame>> {
    let mut result = frames.to_vec();
    let mut sorted: Vec<usize> = indexes
        .iter()
        .copied()
        .filter(|&i| i > 0 && i < result.len())
        .collect();
    sorted.sort_unstable();
    sorted.dedup();

    for index in sorted {
        result.swap(index - 1, index);
    }
    result
}

/// Moves the selected frames one position right.
pub fn move_right(frames: &[Arc<EditorFrame>], indexes: &[usize]) -> Vec<Arc<EditorFrame>> {
    let mut result = frames.to_vec();
    let mut sorted: Vec<usize> = indexes
        .iter()
        .copied()
        .filter(|&i| i + 1 < result.len())
        .collect();
    sorted.sort_unstable_by(|a, b| b.cmp(a));
    sorted.dedup();

    for index in sorted {
        result.swap(index, index + 1);
    }
    result
}

// ─── Delay ────────────────────────────────────────────────────────────────────

/// Sets the selected frames' delay to a fixed value.
pub fn set_delay(
    frames: &[Arc<EditorFrame>],
    indexes: &[usize],
    delay_ms: i32,
) -> Vec<Arc<EditorFrame>> {
    map_selected(frames, indexes, |f| f.with_delay(clamp_delay(delay_ms)))
}

/// Adds `delta_ms` to the selected frames' delay.
pub fn adjust_delay(
    frames: &[Arc<EditorFrame>],
    indexes: &[usize],
    delta_ms: i32,
) -> Vec<Arc<EditorFrame>> {
    map_selected(frames, indexes, |f| f.with_delay(clamp_delay(f.delay + delta_ms)))
}

/// Scales the selected frames' delay by a percentage. 50% doubles the speed.
pub fn scale_delay(
    frames: &[Arc<EditorFrame>],
    indexes: &[usize],
    percent: f64,
) -> Vec<Arc<EditorFrame>> {
    if percent <= 0.0 {
        return frames.to_vec();
    }
    map_selected(frames, indexes, |f| {
        f.with_delay(clamp_delay(round_half_even(f.delay as f64 * percent / 100.0)))
    })
}

/// Sets every frame to a uniform delay for the target frame rate.
pub fn set_fps(frames: &[Arc<EditorFrame>], fps: i32) -> Vec<Arc<EditorFrame>> {
    let delay = clamp_delay(round_half_even(1000.0 / fps.max(1) as f64));
    frames.iter().map(|f| f.with_delay(delay)).collect()
}

// ─── Frame reduction ──────────────────────────────────────────────────────────

/// Keeps `keep` frames, drops `remove`, repeating. The most direct way to cut file size.
pub fn reduce(
    frames: &[Arc<EditorFrame>],
    keep: usize,
    remove_count: usize,
    delay_mode: DelayMergeMode,
) -> Vec<Arc<EditorFrame>> {
    let keep = keep.max(1);
    let remove_count = remove_count.max(1);

    if frames.len() <= keep + 1 {
        return frames.to_vec();
    }

    let mut to_remove = Vec::new();
    let mut i = keep;
    while i < frames.len() {
        for r in 0..remove_count {
            if i + r < frames.len() {
                to_remove.push(i + r);
            }
        }
        i += keep + remove_count;
    }

    // Keep the last frame; the animation should not cut off abruptly.
    to_remove.retain(|&x| x != frames.len() - 1);
    if to_remove.is_empty() || to_remove.len() >= frames.len() {
        return frames.to_vec();
    }

    remove(frames, &to_remove, delay_mode)
}

// ─── Duplicate frames ─────────────────────────────────────────────────────────

/// Drops consecutive similar frames.
pub fn remove_duplicates(
    frames: &[Arc<EditorFrame>],
    similarity: f64,
    removal: DuplicateRemoval,
    delay_mode: DelayMergeMode,
    keep_frames_with_input: bool,
) -> Vec<Arc<EditorFrame>> {
    if frames.len() < 2 {
        return frames.to_vec();
    }

    let threshold = similarity.clamp(0.0, 100.0);
    let mut to_remove = Vec::new();
    let mut last_kept = 0usize;

    for i in 1..frames.len() {
        if keep_frames_with_input && frames[i].input.has_any_input() {
            last_kept = i;
            continue;
        }

        // Compare against the last kept frame; otherwise slowly drifting changes
        // are never noticed.
        if similarity_of(&frames[last_kept].pixels_uncached(), &frames[i].pixels_uncached()) < threshold
        {
            last_kept = i;
            continue;
        }

        to_remove.push(if removal == DuplicateRemoval::First {
            last_kept
        } else {
            i
        });

        if removal == DuplicateRemoval::First {
            last_kept = i;
        }
    }

    if to_remove.is_empty() {
        frames.to_vec()
    } else {
        remove(frames, &to_remove, delay_mode)
    }
}

/// From the end, finds a frame similar enough to the first and drops it and the
/// rest, so the loop closes seamlessly.
pub fn smooth_loop(
    frames: &[Arc<EditorFrame>],
    similarity: f64,
    minimum_frames: usize,
) -> Vec<Arc<EditorFrame>> {
    if frames.len() <= minimum_frames {
        return frames.to_vec();
    }

    let threshold = similarity.clamp(0.0, 100.0);
    let first = frames[0].pixels_uncached();

    for i in (minimum_frames..frames.len()).rev() {
        if similarity_of(&first, &frames[i].pixels_uncached()) < threshold {
            continue;
        }

        // Frame i is nearly identical to the first: the loop already wraps there,
        // so i and everything after it is redundant. Keep 0..i-1.
        return trim(frames, 0, i - 1);
    }

    frames.to_vec()
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Percentage of matching pixels between two frames (0-100).
pub fn similarity_of(a: &[u8], b: &[u8]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let pixel_count = a.len() / 4;
    let mut same = 0usize;
    for p in 0..pixel_count {
        let i = p * 4;
        if a[i..i + 4] == b[i..i + 4] {
            same += 1;
        }
    }

    same as f64 * 100.0 / pixel_count as f64
}

fn map_selected(
    frames: &[Arc<EditorFrame>],
    indexes: &[usize],
    map: impl Fn(&Arc<EditorFrame>) -> Arc<EditorFrame>,
) -> Vec<Arc<EditorFrame>> {
    let selected: std::collections::HashSet<usize> = indexes
        .iter()
        .copied()
        .filter(|&i| i < frames.len())
        .collect();
    if selected.is_empty() {
        return frames.to_vec();
    }

    frames
        .iter()
        .enumerate()
        .map(|(i, f)| if selected.contains(&i) { map(f) } else { Arc::clone(f) })
        .collect()
}

/// Spreads accumulated delay evenly over the remaining frames.
fn distribute_delay(frames: &mut [Arc<EditorFrame>], total_ms: i32) {
    if frames.is_empty() || total_ms <= 0 {
        return;
    }

    let count = frames.len() as i32;
    let share = total_ms / count;
    let remainder = total_ms % count;

    for (i, slot) in frames.iter_mut().enumerate() {
        let extra = share + if (i as i32) < remainder { 1 } else { 0 };
        if extra > 0 {
            *slot = slot.with_delay(clamp_delay(slot.delay + extra));
        }
    }
}

/// `Math.Round` for a positive value: round half to even.
fn round_half_even(value: f64) -> i32 {
    let floor = value.floor();
    let frac = value - floor;
    if (frac - 0.5).abs() < f64::EPSILON {
        if (floor as i64) % 2 == 0 {
            floor as i32
        } else {
            (floor + 1.0) as i32
        }
    } else {
        value.round() as i32
    }
}
