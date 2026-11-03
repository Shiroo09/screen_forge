//! Key → on-screen badge label. Port of `ScreenForge/Gif/Input/KeyLabels.cs`.
//!
//! The C# version switches on a WPF `Key` enum; here the `Key` is already
//! normalized to its name string (e.g. "LeftCtrl", "D5"), so the mapping is the
//! same table without the framework type.

/// A key's badge label, or `None` for keys that should not be shown.
pub fn describe(key: &str) -> Option<String> {
    let fixed = match key {
        "LeftCtrl" | "RightCtrl" => Some("Ctrl"),
        "LeftShift" | "RightShift" => Some("Shift"),
        "LeftAlt" | "RightAlt" | "System" => Some("Alt"),
        "LWin" | "RWin" => Some("Win"),
        "Return" => Some("Enter"),
        "Back" => Some("Backspace"),
        "Delete" => Some("Del"),
        "Insert" => Some("Ins"),
        "Tab" => Some("Tab"),
        "Space" => Some("Space"),
        "Escape" => Some("Esc"),
        "Home" => Some("Home"),
        "End" => Some("End"),
        "PageUp" => Some("PgUp"),
        "PageDown" => Some("PgDn"),
        "Left" => Some("←"),
        "Right" => Some("→"),
        "Up" => Some("↑"),
        "Down" => Some("↓"),
        "OemPlus" | "Add" => Some("+"),
        "OemMinus" | "Subtract" => Some("-"),
        "OemComma" => Some(","),
        "OemPeriod" | "Decimal" => Some("."),
        "F1" => Some("F1"),
        "F2" => Some("F2"),
        "F3" => Some("F3"),
        "F4" => Some("F4"),
        "F5" => Some("F5"),
        "F6" => Some("F6"),
        "F7" => Some("F7"),
        "F8" => Some("F8"),
        "F9" => Some("F9"),
        "F10" => Some("F10"),
        "F11" => Some("F11"),
        "F12" => Some("F12"),
        _ => None,
    };
    if let Some(label) = fixed {
        return Some(label.to_string());
    }

    let bytes = key.as_bytes();
    // Single letters (A-Z) show their own name.
    if bytes.len() == 1 && bytes[0].is_ascii_uppercase() {
        return Some(key.to_string());
    }
    // D0..D9 → "0".."9"
    if bytes.len() == 2 && bytes[0] == b'D' && bytes[1].is_ascii_digit() {
        return Some((bytes[1] as char).to_string());
    }
    // NumPad0..NumPad9 → "0".."9"
    if let Some(digit) = key.strip_prefix("NumPad") {
        if digit.len() == 1 && digit.as_bytes()[0].is_ascii_digit() {
            return Some(digit.to_string());
        }
    }

    None
}

/// Orders held keys readably: modifiers first, then normal keys, deduplicated.
pub fn order(keys: &[String]) -> Vec<String> {
    let mut labelled: Vec<(i32, String)> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for key in keys {
        let Some(label) = describe(key) else {
            continue;
        };
        if !seen.insert(label.clone()) {
            continue;
        }
        labelled.push((modifier_rank(&label), label));
    }

    // Sort by (rank, label) — matches OrderBy(rank).ThenBy(label, Ordinal).
    labelled.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    labelled.into_iter().map(|(_, l)| l).collect()
}

fn modifier_rank(label: &str) -> i32 {
    match label {
        "Ctrl" => 0,
        "Alt" => 1,
        "Shift" => 2,
        "Win" => 3,
        _ => 4,
    }
}
