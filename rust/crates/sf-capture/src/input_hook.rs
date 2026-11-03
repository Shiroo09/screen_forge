//! Low-level mouse/keyboard hook. Port of `ScreenForge/Gif/Input/InputHook.cs`.
//!
//! The gesture state machine (`GestureState`) is pure and unit-tested; the hook
//! that feeds it is a thin Win32 layer, so a win32 failure never affects the
//! state logic. Mouse and keyboard are separate hooks — one being off costs
//! nothing for the other, same as the C# original.

use crate::keys::MouseButtons;

/// Low-level mouse events.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MouseEventType {
    Move,
    LeftDown,
    LeftUp,
    RightDown,
    RightUp,
    MiddleDown,
    MiddleUp,
    ExtraDown,
    ExtraUp,
    Wheel,
    WheelHorizontal,
    DragStart,
    DragEnd,
    DoubleClick,
}

/// A single mouse event in virtual-screen pixel coordinates.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MouseGesture {
    pub kind: MouseEventType,
    pub x: i32,
    pub y: i32,
    pub buttons: MouseButtons,
    pub wheel_delta: i32,
}

/// A single keyboard event.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KeyGesture {
    pub vk: u32,
    pub modifiers: u32,
    pub is_down: bool,
}

// Win32 modifier bits, matching the values the hotkey API uses.
pub const MOD_ALT: u32 = 0x1;
pub const MOD_CONTROL: u32 = 0x2;
pub const MOD_SHIFT: u32 = 0x4;
pub const MOD_WIN: u32 = 0x8;

/// Windows mouse messages the hook reacts to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MouseMessage {
    Move,
    LeftDown,
    LeftUp,
    LeftDblClk,
    RightDown,
    RightUp,
    RightDblClk,
    MiddleDown,
    MiddleUp,
    MiddleDblClk,
    XButtonDown { button1: bool },
    XButtonUp { button1: bool },
    Wheel { delta: i32 },
    WheelHorizontal { delta: i32 },
}

/// The gesture state machine: turns a raw mouse message stream into gestures,
/// tracking held buttons, drag start/end and double-click.
pub struct GestureState {
    buttons: MouseButtons,
    last_click_ms: i64,
    drag_start_x: i32,
    drag_start_y: i32,
    dragging: bool,
    drag_threshold_x: i32,
    drag_threshold_y: i32,
    double_click_ms: i64,
}

impl GestureState {
    pub fn new(drag_threshold_x: i32, drag_threshold_y: i32, double_click_ms: i64) -> Self {
        Self {
            buttons: MouseButtons::default(),
            last_click_ms: 0,
            drag_start_x: 0,
            drag_start_y: 0,
            dragging: false,
            drag_threshold_x: drag_threshold_x.max(1),
            drag_threshold_y: drag_threshold_y.max(1),
            double_click_ms,
        }
    }

    pub fn current_buttons(&self) -> MouseButtons {
        self.buttons
    }

    /// Processes one message and returns the gesture to emit, if any.
    pub fn process(&mut self, message: MouseMessage, x: i32, y: i32, now_ms: i64) -> Option<MouseGesture> {
        let emit = |kind, buttons, wheel_delta| {
            Some(MouseGesture {
                kind,
                x,
                y,
                buttons,
                wheel_delta,
            })
        };

        match message {
            MouseMessage::Move => {
                if !self.dragging && self.buttons.contains(MouseButtons::LEFT) {
                    let dx = (x - self.drag_start_x).abs();
                    let dy = (y - self.drag_start_y).abs();
                    if dx > self.drag_threshold_x || dy > self.drag_threshold_y {
                        self.dragging = true;
                        return emit(MouseEventType::DragStart, self.buttons, 0);
                    }
                }
                emit(MouseEventType::Move, self.buttons, 0)
            }

            MouseMessage::LeftDown => {
                self.buttons.insert(MouseButtons::LEFT);
                self.drag_start_x = x;
                self.drag_start_y = y;
                self.register_click(now_ms);
                emit(MouseEventType::LeftDown, self.buttons, 0)
            }
            MouseMessage::LeftUp => {
                let mut out = None;
                if self.dragging {
                    out = emit(MouseEventType::DragEnd, self.buttons, 0);
                    self.dragging = false;
                }
                self.buttons.remove(MouseButtons::LEFT);
                // A drag end already reported; otherwise report the release.
                out.or_else(|| emit(MouseEventType::LeftUp, self.buttons, 0))
            }

            MouseMessage::RightDown => {
                self.buttons.insert(MouseButtons::RIGHT);
                self.register_click(now_ms);
                emit(MouseEventType::RightDown, self.buttons, 0)
            }
            MouseMessage::RightUp => {
                self.buttons.remove(MouseButtons::RIGHT);
                emit(MouseEventType::RightUp, self.buttons, 0)
            }

            MouseMessage::MiddleDown => {
                self.buttons.insert(MouseButtons::MIDDLE);
                self.register_click(now_ms);
                emit(MouseEventType::MiddleDown, self.buttons, 0)
            }
            MouseMessage::MiddleUp => {
                self.buttons.remove(MouseButtons::MIDDLE);
                emit(MouseEventType::MiddleUp, self.buttons, 0)
            }

            MouseMessage::XButtonDown { button1 } => {
                self.buttons.insert(if button1 { MouseButtons::EXTRA1 } else { MouseButtons::EXTRA2 });
                self.register_click(now_ms);
                emit(MouseEventType::ExtraDown, self.buttons, 0)
            }
            MouseMessage::XButtonUp { button1 } => {
                self.buttons.remove(if button1 { MouseButtons::EXTRA1 } else { MouseButtons::EXTRA2 });
                emit(MouseEventType::ExtraUp, self.buttons, 0)
            }

            MouseMessage::LeftDblClk | MouseMessage::RightDblClk | MouseMessage::MiddleDblClk => {
                emit(MouseEventType::DoubleClick, self.buttons, 0)
            }

            MouseMessage::Wheel { delta } => emit(MouseEventType::Wheel, self.buttons, delta),
            MouseMessage::WheelHorizontal { delta } => {
                emit(MouseEventType::WheelHorizontal, self.buttons, delta)
            }
        }
    }

    fn register_click(&mut self, now_ms: i64) {
        self.last_click_ms = now_ms;
    }

    /// True when two clicks land within the double-click window.
    pub fn is_double_click(&self, now_ms: i64) -> bool {
        now_ms - self.last_click_ms <= self.double_click_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> GestureState {
        GestureState::new(4, 4, 500)
    }

    #[test]
    fn left_down_tracks_button() {
        let mut s = state();
        let g = s.process(MouseMessage::LeftDown, 10, 10, 0).unwrap();
        assert_eq!(g.kind, MouseEventType::LeftDown);
        assert!(s.current_buttons().contains(MouseButtons::LEFT));

        s.process(MouseMessage::LeftUp, 10, 10, 10);
        assert!(!s.current_buttons().contains(MouseButtons::LEFT));
    }

    #[test]
    fn move_beyond_threshold_starts_drag() {
        let mut s = state();
        s.process(MouseMessage::LeftDown, 100, 100, 0);
        // Small move: still a Move.
        let g = s.process(MouseMessage::Move, 102, 100, 5).unwrap();
        assert_eq!(g.kind, MouseEventType::Move);
        // Past the threshold: DragStart.
        let g = s.process(MouseMessage::Move, 110, 100, 10).unwrap();
        assert_eq!(g.kind, MouseEventType::DragStart);
    }

    #[test]
    fn left_up_after_drag_reports_drag_end() {
        let mut s = state();
        s.process(MouseMessage::LeftDown, 0, 0, 0);
        s.process(MouseMessage::Move, 50, 0, 5);
        let g = s.process(MouseMessage::LeftUp, 50, 0, 10).unwrap();
        assert_eq!(g.kind, MouseEventType::DragEnd);
    }

    #[test]
    fn wheel_carries_delta() {
        let mut s = state();
        let g = s.process(MouseMessage::Wheel { delta: -120 }, 5, 5, 0).unwrap();
        assert_eq!(g.kind, MouseEventType::Wheel);
        assert_eq!(g.wheel_delta, -120);
    }

    #[test]
    fn extra_buttons_map_to_extra1_and_extra2() {
        let mut s = state();
        s.process(MouseMessage::XButtonDown { button1: true }, 0, 0, 0);
        assert!(s.current_buttons().contains(MouseButtons::EXTRA1));
        s.process(MouseMessage::XButtonUp { button1: true }, 0, 0, 0);

        s.process(MouseMessage::XButtonDown { button1: false }, 0, 0, 0);
        assert!(s.current_buttons().contains(MouseButtons::EXTRA2));
    }

    #[test]
    fn double_click_window() {
        let mut s = state();
        s.process(MouseMessage::LeftDown, 0, 0, 1000);
        assert!(s.is_double_click(1200));
        assert!(!s.is_double_click(2000));
    }
}
