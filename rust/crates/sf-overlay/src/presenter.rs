//! Presenter mode: tool state and the laser trail. Port of the model in
//! `ScreenForge/Presenter/PresenterTool.cs`, `PresenterService.cs` (tool toggling)
//! and `LaserTrail.cs` (fading trail).
//!
//! Pure state only: which tool is active, and the points the laser has visited
//! with their remaining fade. The overlay window renders from this.

/// The presenter tools. Mirrors `ScreenForge.Presenter.PresenterTool`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PresenterTool {
    #[default]
    None,
    Pen,
    Laser,
    Rectangle,
    Ellipse,
    Line,
    Arrow,
}

impl PresenterTool {
    /// True for tools that start a stroke on press (as opposed to the laser,
    /// which tracks motion continuously).
    pub fn is_drawing(self) -> bool {
        matches!(
            self,
            PresenterTool::Pen
                | PresenterTool::Rectangle
                | PresenterTool::Ellipse
                | PresenterTool::Line
                | PresenterTool::Arrow
        )
    }

    pub fn name(self) -> &'static str {
        match self {
            PresenterTool::None => "None",
            PresenterTool::Pen => "Pen",
            PresenterTool::Laser => "Laser",
            PresenterTool::Rectangle => "Rectangle",
            PresenterTool::Ellipse => "Ellipse",
            PresenterTool::Line => "Line",
            PresenterTool::Arrow => "Arrow",
        }
    }
}

/// A point on the laser trail with the time it was added (ms).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TrailPoint {
    pub x: f32,
    pub y: f32,
    pub added_ms: i64,
}

/// The laser trail: points that fade out after `fade_ms`.
pub struct LaserTrail {
    points: Vec<TrailPoint>,
    fade_ms: i64,
}

impl LaserTrail {
    pub fn new(fade_ms: i64) -> Self {
        Self {
            points: Vec::new(),
            fade_ms: fade_ms.max(1),
        }
    }

    pub fn fade_ms(&self) -> i64 {
        self.fade_ms
    }

    pub fn set_fade_ms(&mut self, fade_ms: i64) {
        self.fade_ms = fade_ms.max(1);
    }

    /// Adds a point to the trail.
    pub fn push(&mut self, x: f32, y: f32, now_ms: i64) {
        self.points.push(TrailPoint {
            x,
            y,
            added_ms: now_ms,
        });
    }

    /// Drops points older than the fade window. Call before reading `visible`.
    pub fn expire(&mut self, now_ms: i64) {
        let cutoff = now_ms - self.fade_ms;
        self.points.retain(|p| p.added_ms >= cutoff);
    }

    /// The visible points with their alpha (1 at newest, 0 at the fade edge).
    pub fn visible(&self, now_ms: i64) -> Vec<(f32, f32, f32)> {
        self.points
            .iter()
            .map(|p| {
                let age = (now_ms - p.added_ms).max(0) as f32;
                let alpha = (1.0 - age / self.fade_ms as f32).clamp(0.0, 1.0);
                (p.x, p.y, alpha)
            })
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    pub fn clear(&mut self) {
        self.points.clear();
    }
}

/// Current presenter session state: active tool and spotlight toggle.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct PresenterState {
    pub tool: PresenterTool,
    pub spotlight: bool,
    /// Draft start point while a drawing tool is in progress.
    pub draft_from: Option<(f32, f32)>,
}

impl PresenterState {
    /// Toggles `tool`, or clears it if already active. The laser never blocks
    /// other tools (matches the C# toggle behaviour).
    pub fn toggle_tool(&mut self, tool: PresenterTool) {
        self.tool = if self.tool == tool {
            PresenterTool::None
        } else {
            tool
        };
    }

    pub fn toggle_spotlight(&mut self) {
        self.spotlight = !self.spotlight;
    }

    /// Esc / cancel: everything off.
    pub fn cancel(&mut self) {
        self.tool = PresenterTool::None;
        self.spotlight = false;
        self.draft_from = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggling_same_tool_clears_it() {
        let mut s = PresenterState::default();
        s.toggle_tool(PresenterTool::Pen);
        assert_eq!(s.tool, PresenterTool::Pen);
        s.toggle_tool(PresenterTool::Pen);
        assert_eq!(s.tool, PresenterTool::None);
    }

    #[test]
    fn switching_tool_replaces() {
        let mut s = PresenterState::default();
        s.toggle_tool(PresenterTool::Pen);
        s.toggle_tool(PresenterTool::Arrow);
        assert_eq!(s.tool, PresenterTool::Arrow);
    }

    #[test]
    fn trail_points_fade_out() {
        let mut trail = LaserTrail::new(700);
        trail.push(10.0, 10.0, 0);
        trail.push(20.0, 20.0, 500);

        let visible = trail.visible(500);
        assert_eq!(visible.len(), 2);
        // The newest point is at full alpha, the older one faded.
        assert!((visible[1].2 - 1.0).abs() < 1e-4);
        assert!(visible[0].2 < 1.0);

        // After the fade window, the oldest is gone.
        trail.expire(800);
        assert_eq!(trail.visible(800).len(), 1);
        trail.expire(1300);
        assert!(trail.is_empty());
    }

    #[test]
    fn cancel_clears_everything() {
        let mut s = PresenterState::default();
        s.toggle_tool(PresenterTool::Laser);
        s.toggle_spotlight();
        assert!(s.spotlight);

        s.cancel();
        assert_eq!(s.tool, PresenterTool::None);
        assert!(!s.spotlight);
    }
}
