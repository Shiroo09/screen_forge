//! Region-selection state machine. The pure half of the capture overlay:
//! turns a mouse drag into a normalized rectangle. Dragging in any direction
//! (right-to-left, bottom-to-top) yields a positive-size rect, which is what a
//! crop needs.

/// Rectangle as `(x, y, width, height)`, always with non-negative width/height.
pub type SelectionRect = (i32, i32, i32, i32);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Idle,
    Dragging,
    Done,
}

/// Tracks a single drag from press to release.
pub struct SelectionState {
    phase: Phase,
    start: (i32, i32),
    current: (i32, i32),
}

impl Default for SelectionState {
    fn default() -> Self {
        Self::new()
    }
}

impl SelectionState {
    pub fn new() -> Self {
        Self {
            phase: Phase::Idle,
            start: (0, 0),
            current: (0, 0),
        }
    }

    pub fn is_dragging(&self) -> bool {
        self.phase == Phase::Dragging
    }

    /// Begins a selection at the press point.
    pub fn begin(&mut self, x: i32, y: i32) {
        self.start = (x, y);
        self.current = (x, y);
        self.phase = Phase::Dragging;
    }

    /// Updates the drag end during a move.
    pub fn update(&mut self, x: i32, y: i32) {
        if self.phase == Phase::Dragging {
            self.current = (x, y);
        }
    }

    /// The live rectangle from `start` to `current` (zero-size before any drag).
    pub fn current_rect(&self) -> SelectionRect {
        normalize(self.start, self.current)
    }

    /// Ends the selection and returns the rectangle if it is at least `min`×`min`.
    pub fn finish(&mut self, min: i32) -> Option<SelectionRect> {
        if self.phase != Phase::Dragging {
            return None;
        }
        self.phase = Phase::Done;

        let rect = normalize(self.start, self.current);
        if rect.2 >= min && rect.3 >= min {
            Some(rect)
        } else {
            None
        }
    }

    /// True when the selection was too small and should be treated as a click.
    pub fn is_click(&self, min: i32) -> bool {
        let r = self.current_rect();
        r.2 < min || r.3 < min
    }
}

fn normalize(start: (i32, i32), end: (i32, i32)) -> SelectionRect {
    let x = start.0.min(end.0);
    let y = start.1.min(end.1);
    let w = (end.0 - start.0).abs();
    let h = (end.1 - start.1).abs();
    (x, y, w, h)
}

/// Bir seçimin hangi tutamacının tutulduğu. `Inside` taşımadır.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Handle {
    NW,
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
    Inside,
}

impl Handle {
    fn moves_left(self) -> bool {
        matches!(self, Handle::NW | Handle::W | Handle::SW)
    }
    fn moves_right(self) -> bool {
        matches!(self, Handle::NE | Handle::E | Handle::SE)
    }
    fn moves_top(self) -> bool {
        matches!(self, Handle::NW | Handle::N | Handle::NE)
    }
    fn moves_bottom(self) -> bool {
        matches!(self, Handle::SW | Handle::S | Handle::SE)
    }
}

/// `SelectDrag::begin` ayarları: kavrama toleransı, en küçük boyut ve seçimin
/// içinde kalması gereken sınırlar.
#[derive(Clone, Copy)]
pub struct DragCfg {
    pub grab: i32,
    pub min: i32,
    /// (x, y, w, h) — seçim bu dikdörtgenin dışına taşamaz.
    pub bounds: SelectionRect,
}

/// Biten bir seçimi tutamaçtan boyutlandırma veya içinden taşıma.
///
/// Kenar tutamaçları çağrı sırasına göre önceliklidir: köşe, kenardan önce
/// denenir, böylece köşeye yakın bir tıklama kenar boyutlandırmasına kaymaz.
pub struct SelectDrag {
    handle: Handle,
    origin: SelectionRect,
    start: (i32, i32),
    cfg: DragCfg,
}

impl SelectDrag {
    /// `point` bir tutamacın üstündeyse (ya da seçimin içindeyse) sürüklemeyi
    /// başlatır; değilse `None` döner ve çağıran yeni seçim başlatabilir.
    pub fn begin(rect: SelectionRect, point: (i32, i32), cfg: DragCfg) -> Option<Self> {
        let handle = handle_at(rect, point, cfg.grab)?;
        Some(Self {
            handle,
            origin: rect,
            start: point,
            cfg,
        })
    }

    /// Sürüklemenin şu andaki seçimi. Taşıma sınırlara kırpılır; boyutlandırma
    /// en küçük boyutun altına inemez.
    pub fn rect(&self, point: (i32, i32)) -> SelectionRect {
        let (ox, oy, ow, oh) = self.origin;
        let (dx, dy) = (point.0 - self.start.0, point.1 - self.start.1);
        let (mut l, mut t) = (ox, oy);
        let (mut r, mut b) = (ox + ow, oy + oh);

        if self.handle == Handle::Inside {
            // Taşımada boyut değişmez, yalnız konum kırpılır.
            let (bx, by, bw, bh) = self.cfg.bounds;
            l = (l + dx).clamp(bx, (bx + bw - ow).max(bx));
            t = (t + dy).clamp(by, (by + bh - oh).max(by));
            return (l, t, ow, oh);
        }

        if self.handle.moves_left() {
            l += dx;
        }
        if self.handle.moves_right() {
            r += dx;
        }
        if self.handle.moves_top() {
            t += dy;
        }
        if self.handle.moves_bottom() {
            b += dy;
        }

        let min = self.cfg.min;
        let (bx, by, bw, bh) = self.cfg.bounds;
        // Ters çevirmek yerine en küçük boyutta durdur: sürüklerken seçim
        // kullanıcının elinde "kağıt gibi" dönmemeli.
        if self.handle.moves_left() {
            l = l.clamp(bx, (r - min).max(bx));
        } else {
            r = r.clamp(l + min, bx + bw);
        }
        if self.handle.moves_top() {
            t = t.clamp(by, (b - min).max(by));
        } else {
            b = b.clamp(t + min, by + bh);
        }
        (l, t, (r - l).max(min), (b - t).max(min))
    }
}

/// `point` hangi tutamacın üstünde. Köşeler kenarlardan önce denenir.
pub fn handle_at(rect: SelectionRect, point: (i32, i32), grab: i32) -> Option<Handle> {
    let (x, y, w, h) = rect;
    let (px, py) = point;
    let (l, t, r, b) = (x, y, x + w, y + h);
    let near_l = (px - l).abs() <= grab;
    let near_r = (px - r).abs() <= grab;
    let near_t = (py - t).abs() <= grab;
    let near_b = (py - b).abs() <= grab;
    let within_x = px >= l - grab && px <= r + grab;
    let within_y = py >= t - grab && py <= b + grab;

    if near_l && near_t {
        return Some(Handle::NW);
    }
    if near_r && near_t {
        return Some(Handle::NE);
    }
    if near_r && near_b {
        return Some(Handle::SE);
    }
    if near_l && near_b {
        return Some(Handle::SW);
    }
    if near_l && within_y {
        return Some(Handle::W);
    }
    if near_r && within_y {
        return Some(Handle::E);
    }
    if near_t && within_x {
        return Some(Handle::N);
    }
    if near_b && within_x {
        return Some(Handle::S);
    }
    if px > l && px < r && py > t && py < b {
        return Some(Handle::Inside);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_down_right() {
        let mut s = SelectionState::new();
        s.begin(10, 20);
        s.update(110, 70);
        assert_eq!(s.current_rect(), (10, 20, 100, 50));
        assert_eq!(s.finish(2), Some((10, 20, 100, 50)));
    }

    #[test]
    fn drag_up_left_normalizes() {
        let mut s = SelectionState::new();
        s.begin(110, 70);
        s.update(10, 20);
        assert_eq!(s.current_rect(), (10, 20, 100, 50));
        assert_eq!(s.finish(2), Some((10, 20, 100, 50)));
    }

    #[test]
    fn tiny_drag_rejected_as_click() {
        let mut s = SelectionState::new();
        s.begin(50, 50);
        s.update(51, 51);
        assert!(s.is_click(4));
        assert_eq!(s.finish(4), None);
    }

    #[test]
    fn update_before_begin_is_ignored() {
        let mut s = SelectionState::new();
        s.update(10, 10);
        assert_eq!(s.current_rect(), (0, 0, 0, 0));
    }

    #[test]
    fn finish_without_drag_is_none() {
        let mut s = SelectionState::new();
        assert_eq!(s.finish(2), None);
    }

    const CFG: DragCfg = DragCfg {
        grab: 6,
        min: 8,
        bounds: (0, 0, 1000, 1000),
    };

    #[test]
    fn corner_beats_edge_when_both_match() {
        // Köşeye 3px uzaklıkta: hem sol hem üst kenar eşleşir, köşe kazanmalı.
        assert_eq!(
            handle_at((100, 100, 200, 200), (103, 103), 6),
            Some(Handle::NW)
        );
        assert_eq!(
            handle_at((100, 100, 200, 200), (297, 297), 6),
            Some(Handle::SE)
        );
    }

    #[test]
    fn inside_is_move_not_resize() {
        assert_eq!(
            handle_at((100, 100, 200, 200), (200, 200), 6),
            Some(Handle::Inside)
        );
        // Uzakta hiçbir tutamaç yok: yeni seçim başlatılabilir.
        assert_eq!(handle_at((100, 100, 200, 200), (500, 500), 6), None);
    }

    #[test]
    fn dragging_corner_resizes_only_that_corner() {
        let d = SelectDrag::begin((100, 100, 200, 200), (100, 100), CFG).unwrap();
        let r = d.rect((60, 40));
        assert_eq!(r, (60, 40, 240, 260), "karşı köşe sabit kalmalı");
    }

    #[test]
    fn resize_cannot_shrink_below_min() {
        let d = SelectDrag::begin((100, 100, 200, 200), (100, 100), CFG).unwrap();
        let r = d.rect((295, 295));
        assert_eq!(r, (292, 292, 8, 8), "en küçük boyutta durmalı, ters dönmemeli");
    }

    #[test]
    fn move_keeps_the_size() {
        let d = SelectDrag::begin((100, 100, 200, 200), (200, 200), CFG).unwrap();
        let r = d.rect((250, 230));
        assert_eq!(r, (150, 130, 200, 200));
    }

    #[test]
    fn move_clamps_at_the_bounds() {
        let d = SelectDrag::begin((100, 100, 200, 200), (200, 200), CFG).unwrap();
        assert_eq!(d.rect((-500, -500)), (0, 0, 200, 200));
        assert_eq!(d.rect((5000, 5000)), (800, 800, 200, 200));
    }

    #[test]
    fn begin_returns_none_away_from_the_selection() {
        assert!(SelectDrag::begin((100, 100, 200, 200), (600, 600), CFG).is_none());
    }
}
