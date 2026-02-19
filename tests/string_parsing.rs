use evdev::KeyCode;
use evdev_hotkey::{Hotkey, HotkeySequence};

#[test]
fn parses_hotkey_with_aliases_case_insensitive() {
    let hotkey = "ctrl+Win+return".parse::<Hotkey>().unwrap();
    assert_eq!(hotkey.key(), KeyCode::KEY_ENTER);
    assert_eq!(
        hotkey.modifiers(),
        &[KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTMETA]
    );
}

#[test]
fn display_round_trips_hotkey() {
    let parsed = "Super+Shift+A".parse::<Hotkey>().unwrap();
    let round_trip = parsed.to_string().parse::<Hotkey>().unwrap();
    assert_eq!(parsed, round_trip);
}

#[test]
fn parses_hotkey_sequence() {
    let sequence = "Ctrl+K, Ctrl+C".parse::<HotkeySequence>().unwrap();
    assert_eq!(sequence.steps().len(), 2);
    assert_eq!(sequence.steps()[0].key(), KeyCode::KEY_K);
    assert_eq!(sequence.steps()[1].key(), KeyCode::KEY_C);
}
