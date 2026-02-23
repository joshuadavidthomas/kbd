use kbd_evdev::EvdevKeyExt;
use kbd_evdev::KeyCodeExt;
use keybound::Key;
use keybound::Modifier;

#[test]
fn key_round_trips_through_evdev_keycode() {
    for key in [
        Key::A,
        Key::Num9,
        Key::F24,
        Key::Enter,
        Key::Left,
        Key::NumpadEnter,
        Key::LeftCtrl,
        Key::RightSuper,
    ] {
        let code = key.to_key_code();
        let reparsed = code.to_key();
        assert_eq!(reparsed, key, "failed for {key:?}");
    }
}

#[test]
fn unknown_evdev_keycode_maps_to_unknown_key() {
    use evdev::KeyCode;
    assert_eq!(KeyCode::KEY_VOLUMEUP.to_key(), Key::Unknown);
}

#[test]
fn modifier_try_from_key_canonicalizes_left_and_right_variants() {
    assert_eq!(Modifier::try_from(Key::LeftCtrl), Ok(Modifier::Ctrl));
    assert_eq!(Modifier::try_from(Key::RightCtrl), Ok(Modifier::Ctrl));
    assert_eq!(Modifier::try_from(Key::LeftShift), Ok(Modifier::Shift));
    assert_eq!(Modifier::try_from(Key::RightShift), Ok(Modifier::Shift));
    assert_eq!(Modifier::try_from(Key::LeftAlt), Ok(Modifier::Alt));
    assert_eq!(Modifier::try_from(Key::RightAlt), Ok(Modifier::Alt));
    assert_eq!(Modifier::try_from(Key::LeftSuper), Ok(Modifier::Super));
    assert_eq!(Modifier::try_from(Key::RightSuper), Ok(Modifier::Super));
    assert_eq!(Modifier::try_from(Key::A), Err(Key::A));
}
