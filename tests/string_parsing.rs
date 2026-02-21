use keybound::Hotkey;
use keybound::HotkeySequence;
use keybound::Key;
use keybound::Modifier;

#[test]
fn parses_hotkey_with_aliases_case_insensitive() {
    let hotkey = "ctrl+Win+return".parse::<Hotkey>().unwrap();
    assert_eq!(hotkey.key(), Key::Enter);
    assert_eq!(hotkey.modifiers(), &[Modifier::Ctrl, Modifier::Super]);
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
    assert_eq!(sequence.steps()[0].key(), Key::K);
    assert_eq!(sequence.steps()[1].key(), Key::C);
}

#[test]
fn parses_extended_key_ranges() {
    let cases = [
        ("F24", Key::F24),
        ("Left", Key::Left),
        ("Delete", Key::Delete),
        ("Backspace", Key::Backspace),
        ("Insert", Key::Insert),
        ("Home", Key::Home),
        ("End", Key::End),
        ("PageUp", Key::PageUp),
        ("PageDown", Key::PageDown),
        ("Numpad1", Key::Numpad1),
        ("NumpadEnter", Key::NumpadEnter),
        ("Equal", Key::Equal),
        ("Minus", Key::Minus),
        ("Comma", Key::Comma),
        ("Slash", Key::Slash),
    ];

    for (input, expected) in cases {
        let hotkey = format!("Ctrl+{input}").parse::<Hotkey>().unwrap();
        assert_eq!(hotkey.key(), expected, "failed parsing {input}");

        let round_trip = hotkey.to_string().parse::<Hotkey>().unwrap();
        assert_eq!(round_trip, hotkey, "failed round-trip for {input}");
    }
}

#[test]
fn new_produces_sorted_modifiers() {
    let hotkey = Hotkey::new(Key::X, vec![Modifier::Alt, Modifier::Ctrl]);

    assert_eq!(hotkey.modifiers(), &[Modifier::Ctrl, Modifier::Alt]);

    let round_trip = hotkey.to_string().parse::<Hotkey>().unwrap();
    assert_eq!(round_trip, hotkey);
}
