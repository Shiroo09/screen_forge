//! Mirrors `ScreenForge.Tests/{GifInputTests,WindowsReservedHotkeysTests}.cs`.

use sf_capture::key_labels;
use sf_capture::overlay::InputOverlayOptions;
use sf_capture::reserved_hotkeys;
use sf_settings::{HotkeyConfig, Modifiers};

// ─── Key labels ───────────────────────────────────────────────────────────────

#[test]
fn describe_maps_keys_to_labels() {
    let cases = [
        ("LeftCtrl", "Ctrl"),
        ("RightCtrl", "Ctrl"),
        ("LeftShift", "Shift"),
        ("A", "A"),
        ("D5", "5"),
        ("NumPad3", "3"),
        ("F7", "F7"),
        ("Return", "Enter"),
        ("Left", "←"),
    ];
    for (key, expected) in cases {
        assert_eq!(
            key_labels::describe(key).as_deref(),
            Some(expected),
            "key {key}"
        );
    }
}

#[test]
fn describe_returns_none_for_unlabelled_keys() {
    assert!(key_labels::describe("None").is_none());
    assert!(key_labels::describe("").is_none());
}

#[test]
fn order_puts_modifiers_first_and_deduplicates() {
    let keys = vec![
        "S".to_string(),
        "LeftShift".to_string(),
        "LeftCtrl".to_string(),
        "RightCtrl".to_string(),
    ];

    let ordered = key_labels::order(&keys);

    // Ctrl held twice but one label; modifiers before the normal key.
    assert_eq!(ordered, vec!["Ctrl", "Shift", "S"]);
}

// ─── Reserved hotkeys ─────────────────────────────────────────────────────────

fn modifiers(flags: &[u8]) -> Modifiers {
    let mut m = Modifiers::empty();
    for &f in flags {
        m.insert(f);
    }
    m
}

#[test]
fn describe_flags_known_windows_chords() {
    let win_shift_s = HotkeyConfig::new(
        modifiers(&[Modifiers::WINDOWS, Modifiers::SHIFT]),
        "S",
    );
    let msg = reserved_hotkeys::describe(&win_shift_s).expect("reserved");
    assert!(msg.contains("Ekran alıntısı"), "got {msg}");

    let alt_tab = HotkeyConfig::new(modifiers(&[Modifiers::ALT]), "Tab");
    let msg = reserved_hotkeys::describe(&alt_tab).expect("reserved");
    assert!(msg.contains("Görev değiştirme"), "got {msg}");
}

#[test]
fn describe_app_defaults_are_not_reserved() {
    assert!(reserved_hotkeys::describe(&HotkeyConfig::alt_shift("S")).is_none());
    assert!(reserved_hotkeys::describe(&HotkeyConfig::alt_shift("T")).is_none());
    assert!(reserved_hotkeys::describe(&HotkeyConfig::default()).is_none());
}

#[test]
fn win_digit_is_taskbar() {
    let cfg = HotkeyConfig::new(modifiers(&[Modifiers::WINDOWS]), "D3");
    let msg = reserved_hotkeys::describe(&cfg).expect("reserved");
    assert!(msg.contains("Görev çubuğu"), "got {msg}");
}

// ─── Overlay ──────────────────────────────────────────────────────────────────

#[test]
fn overlay_defaults_match_screentogif() {
    let o = InputOverlayOptions::default();
    assert!(o.highlight_clicks);
    assert!(!o.highlight_cursor);
    assert!(o.show_keys);
    assert!(o.has_work());
}

#[test]
fn overlay_without_work_reports_none() {
    let o = InputOverlayOptions {
        highlight_clicks: false,
        highlight_cursor: false,
        show_keys: false,
        ..Default::default()
    };
    assert!(!o.has_work());
}
