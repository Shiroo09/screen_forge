//! Global-hotkey modifier flags and their serialized form.
//!
//! C# serializes `[Flags] ModifierKeys` via `JsonStringEnumConverter`, which joins
//! set flags with ", " in ascending bit order ("Alt, Control", never "Control, Alt").
//! That string is in every existing settings file, so both directions must match it.

use serde::ser::Serializer;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub struct Modifiers(pub u8);

impl Modifiers {
    pub const NONE: u8 = 0;
    pub const ALT: u8 = 1;
    pub const CONTROL: u8 = 2;
    pub const SHIFT: u8 = 4;
    pub const WINDOWS: u8 = 8;

    pub fn empty() -> Self {
        Self(0)
    }

    pub fn contains(self, flag: u8) -> bool {
        self.0 & flag != 0
    }

    pub fn insert(&mut self, flag: u8) {
        self.0 |= flag;
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The C# `System.Text.Json` wire form: names joined by ", " in bit order.
    pub fn to_wire(self) -> String {
        let mut parts = Vec::new();
        if self.contains(Self::ALT) {
            parts.push("Alt");
        }
        if self.contains(Self::CONTROL) {
            parts.push("Control");
        }
        if self.contains(Self::SHIFT) {
            parts.push("Shift");
        }
        if self.contains(Self::WINDOWS) {
            parts.push("Windows");
        }
        if parts.is_empty() {
            "None".to_string()
        } else {
            parts.join(", ")
        }
    }

    /// Parses the wire form. Unknown names are ignored, matching the C# converter's
    /// tolerance for flags it does not recognize.
    pub fn from_wire(value: &str) -> Self {
        let mut flags = 0u8;
        for token in value.split(',') {
            match token.trim() {
                "Alt" => flags |= Self::ALT,
                "Control" => flags |= Self::CONTROL,
                "Shift" => flags |= Self::SHIFT,
                "Windows" => flags |= Self::WINDOWS,
                "None" | "" => {}
                _ => {}
            }
        }
        Self(flags)
    }
}

impl Serialize for Modifiers {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_wire())
    }
}

impl<'de> Deserialize<'de> for Modifiers {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Modifiers::from_wire(&s))
    }
}
