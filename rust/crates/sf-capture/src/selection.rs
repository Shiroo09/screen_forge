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
}
