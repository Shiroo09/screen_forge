//! Live-subtitle math. Port of `ScreenForge/Subtitle/SubtitleMath.cs`.
//!
//! Target language, OCR text cleanup, frame-change sampling, fingerprints, OCR
//! downscaling and overlay box placement. No framework types — rectangles are
//! `(x, y, width, height)` tuples.

pub const SAMPLE_COUNT: usize = 48 * 16;
pub const CHANGE_THRESHOLD: i32 = 28;

/// The translation target: the native language, or "tr" when unset.
pub fn target_language(native: Option<&str>) -> String {
    match native {
        Some(n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => "tr".to_string(),
    }
}

/// Collapses blank lines and internal whitespace in OCR output; caps at 2000 chars.
pub fn normalize_ocr(text: Option<&str>) -> Option<String> {
    let text = text?;
    if text.trim().is_empty() {
        return None;
    }

    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut kept = Vec::new();
    for line in normalized.split('\n') {
        let collapsed: Vec<&str> = line.split(' ').filter(|s| !s.is_empty()).collect();
        if !collapsed.is_empty() {
            kept.push(collapsed.join(" "));
        }
    }

    if kept.is_empty() {
        return None;
    }

    let joined = kept.join("\n");
    if joined.chars().count() <= 2000 {
        Some(joined)
    } else {
        Some(joined.chars().take(2000).collect())
    }
}

pub fn same_text(a: Option<&str>, b: Option<&str>) -> bool {
    a == b
}

/// Samples a 48×16 luminance grid from a BGRA frame into `dest`.
pub fn write_samples(bgra: &[u8], width: i32, height: i32, dest: &mut [u8]) {
    dest.iter_mut().for_each(|b| *b = 0);
    if width < 1 || height < 1 || (bgra.len() as i32) < width * height * 4 || dest.is_empty() {
        return;
    }

    let step_x = (width / 48).max(1);
    let step_y = (height / 16).max(1);
    let mut i = 0usize;
    let mut y = 0;
    while y < height && i < dest.len() {
        let row = y * width * 4;
        let mut x = 0;
        while x < width && i < dest.len() {
            let p = (row + x * 4) as usize;
            dest[i] = ((bgra[p] as u32 + bgra[p + 1] as u32 + bgra[p + 2] as u32) / 3) as u8;
            i += 1;
            x += step_x;
        }
        y += step_y;
    }
}

/// How many sample cells changed by more than the noise threshold.
pub fn changed_samples(previous: &[u8], current: &[u8]) -> i32 {
    let n = previous.len().min(current.len());
    let mut changed = 0;
    for i in 0..n {
        if (previous[i] as i32 - current[i] as i32).abs() > CHANGE_THRESHOLD {
            changed += 1;
        }
    }
    changed
}

/// True when two OCR strings are equal or within a small edit distance (jitter).
pub fn nearly_same(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let (al, bl) = (a.chars().count(), b.chars().count());
    if al < 6 || bl < 6 {
        return false;
    }
    if (al as i32 - bl as i32).abs() > 3 {
        return false;
    }
    edit_distance(a, b) <= 2
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (n, m) = (a.len(), b.len());

    let mut prev: Vec<usize> = (0..=m).collect();
    let mut cur = vec![0usize; m + 1];

    for i in 1..=n {
        cur[0] = i;
        for j in 1..=m {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (cur[j - 1] + 1).min(prev[j] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }

    prev[m]
}

/// FNV-1a fingerprint of a 48×16 luminance sample grid. 0 means invalid input.
pub fn fingerprint(bgra: &[u8], width: i32, height: i32) -> u64 {
    if width < 1 || height < 1 || (bgra.len() as i32) < width * height * 4 {
        return 0;
    }

    const FNV_PRIME: u64 = 1099511628211;
    const FNV_OFFSET: u64 = 14695981039346656037;

    let step_x = (width / 48).max(1);
    let step_y = (height / 16).max(1);
    let mut hash = FNV_OFFSET;
    hash ^= width as u32 as u64;
    hash = hash.wrapping_mul(FNV_PRIME);
    hash ^= height as u32 as u64;
    hash = hash.wrapping_mul(FNV_PRIME);

    let mut y = 0;
    while y < height {
        let row = y * width * 4;
        let mut x = 0;
        while x < width {
            let i = (row + x * 4) as usize;
            let lum = bgra[i] as u32 + bgra[i + 1] as u32 + bgra[i + 2] as u32;
            hash ^= lum as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
            x += step_x;
        }
        y += step_y;
    }

    hash
}

/// Downscales an OCR region to at most 1280×240, preserving aspect ratio.
pub fn ocr_size(width: i32, height: i32) -> (i32, i32) {
    const MAX_W: i32 = 1280;
    const MAX_H: i32 = 240;
    if width <= MAX_W && height <= MAX_H {
        return (width.max(1), height.max(1));
    }

    let scale = (MAX_W as f64 / width as f64).min(MAX_H as f64 / height as f64);
    (
        ((width as f64 * scale).round() as i32).max(1),
        ((height as f64 * scale).round() as i32).max(1),
    )
}

/// Nearest-neighbour BGRA downscale. Matches the C# integer-stepping sampler.
pub fn scale_bgra(src: &[u8], sw: i32, sh: i32, dw: i32, dh: i32) -> Vec<u8> {
    let mut dst = vec![0u8; (dw * dh * 4).max(0) as usize];
    if sw < 1 || sh < 1 || dw < 1 || dh < 1 || (src.len() as i32) < sw * sh * 4 {
        return dst;
    }

    for y in 0..dh {
        let sy = y * sh / dh;
        let dst_row = y * dw * 4;
        let src_row = sy * sw * 4;
        for x in 0..dw {
            let sx = x * sw / dw;
            let si = (src_row + sx * 4) as usize;
            let di = (dst_row + x * 4) as usize;
            dst[di] = src[si];
            dst[di + 1] = src[si + 1];
            dst[di + 2] = src[si + 2];
            dst[di + 3] = src[si + 3];
        }
    }

    dst
}

/// Rect as `(x, y, width, height)`.
pub type Rect = (i32, i32, i32, i32);

fn right(r: Rect) -> i32 {
    r.0 + r.2
}

fn bottom(r: Rect) -> i32 {
    r.1 + r.3
}

/// The default subtitle box: centred under the source, flipped above it near the
/// bottom edge, clamped to the screen.
pub fn default_box(source: Rect, screen: Rect) -> Rect {
    let margin = 8;
    let max_w = (screen.2 - margin * 2).max(40);
    let max_h = (screen.3 - margin * 2).max(28);
    let w = 420.min(max_w);
    let h = 56.min(max_h);
    let mut x = source.0 + (source.2 - w) / 2;
    let mut y = bottom(source) + 16;
    if y + h > bottom(screen) - margin {
        y = source.1 - h - 16;
    }
    x = x.clamp(screen.0 + margin, (screen.0 + margin).max(right(screen) - w - margin));
    y = y.clamp(
        screen.1 + margin,
        (screen.1 + margin).max(bottom(screen) - h - margin),
    );
    (x, y, w, h)
}

/// Slides `box` off `blocked` along its shortest overlapping axis.
pub fn push_outside(box_: Rect, blocked: Rect) -> Rect {
    if box_.2 < 1 || box_.3 < 1 || blocked.2 < 1 || blocked.3 < 1 {
        return box_;
    }
    if !intersects(box_, blocked) {
        return box_;
    }

    let left = right(box_) - blocked.0;
    let right_ = right(blocked) - box_.0;
    let up = bottom(box_) - blocked.1;
    let down = bottom(blocked) - box_.1;
    let min = left.min(right_).min(up).min(down);

    if min == left {
        (box_.0 - left, box_.1, box_.2, box_.3)
    } else if min == right_ {
        (box_.0 + right_, box_.1, box_.2, box_.3)
    } else if min == up {
        (box_.0, box_.1 - up, box_.2, box_.3)
    } else {
        (box_.0, box_.1 + down, box_.2, box_.3)
    }
}

fn intersects(a: Rect, b: Rect) -> bool {
    a.0 < right(b) && right(a) > b.0 && a.1 < bottom(b) && bottom(a) > b.1
}

/// Half-open rectangle containment.
pub fn contains(x: i32, y: i32, left: i32, top: i32, width: i32, height: i32) -> bool {
    width > 0 && height > 0 && x >= left && y >= top && x < left + width && y < top + height
}
