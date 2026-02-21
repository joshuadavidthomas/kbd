use evdev::KeyCode;
use keybound::Hotkey;
use keybound::HotkeySequence;

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

#[test]
fn parses_extended_key_ranges() {
    let cases = [
        ("F24", KeyCode::KEY_F24),
        ("Left", KeyCode::KEY_LEFT),
        ("Delete", KeyCode::KEY_DELETE),
        ("Backspace", KeyCode::KEY_BACKSPACE),
        ("Insert", KeyCode::KEY_INSERT),
        ("Home", KeyCode::KEY_HOME),
        ("End", KeyCode::KEY_END),
        ("PageUp", KeyCode::KEY_PAGEUP),
        ("PageDown", KeyCode::KEY_PAGEDOWN),
        ("Numpad1", KeyCode::KEY_KP1),
        ("NumpadEnter", KeyCode::KEY_KPENTER),
        ("Plus", KeyCode::KEY_EQUAL),
        ("Minus", KeyCode::KEY_MINUS),
        ("Comma", KeyCode::KEY_COMMA),
        ("Slash", KeyCode::KEY_SLASH),
    ];

    for (input, expected) in cases {
        let hotkey = format!("Ctrl+{input}").parse::<Hotkey>().unwrap();
        assert_eq!(hotkey.key(), expected, "failed parsing {input}");

        let round_trip = hotkey.to_string().parse::<Hotkey>().unwrap();
        assert_eq!(round_trip, hotkey, "failed round-trip for {input}");
    }
}

#[test]
fn new_canonicalizes_left_right_modifier_variants() {
    let hotkey = Hotkey::new(
        KeyCode::KEY_X,
        vec![KeyCode::KEY_RIGHTCTRL, KeyCode::KEY_RIGHTALT],
    );

    assert_eq!(
        hotkey.modifiers(),
        &[KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTALT]
    );

    let round_trip = hotkey.to_string().parse::<Hotkey>().unwrap();
    assert_eq!(round_trip, hotkey);
}
