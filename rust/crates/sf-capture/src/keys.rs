//! Mouse button bit flags, shared between capture/input and the editor.
//! Matches `ScreenForge.Gif.Input.MouseButtons` values.

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub struct MouseButtons(pub u8);

impl MouseButtons {
    pub const NONE: u8 = 0;
    pub const LEFT: u8 = 1;
    pub const RIGHT: u8 = 2;
    pub const MIDDLE: u8 = 4;
    pub const EXTRA1: u8 = 8;
    pub const EXTRA2: u8 = 16;

    pub fn contains(self, flag: u8) -> bool {
        self.0 & flag != 0
    }

    pub fn insert(&mut self, flag: u8) {
        self.0 |= flag;
    }

    pub fn is_none(self) -> bool {
        self.0 == Self::NONE
    }
}
