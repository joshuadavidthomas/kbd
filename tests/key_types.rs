use evdev::KeyCode;
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
        let code: KeyCode = key.into();
        let reparsed = Key::from(code);
        assert_eq!(reparsed, key, "failed for {key:?}");
    }
}

#[test]
fn unknown_evdev_keycode_maps_to_unknown_key() {
    assert_eq!(Key::from(KeyCode::KEY_VOLUMEUP), Key::Unknown);
}

#[test]
fn modifier_is_derivable_from_left_and_right_key_variants() {
    assert_eq!(Modifier::from_key(Key::LeftCtrl), Some(Modifier::Ctrl));
    assert_eq!(Modifier::from_key(Key::RightCtrl), Some(Modifier::Ctrl));
    assert_eq!(Modifier::from_key(Key::LeftShift), Some(Modifier::Shift));
    assert_eq!(Modifier::from_key(Key::RightShift), Some(Modifier::Shift));
    assert_eq!(Modifier::from_key(Key::LeftAlt), Some(Modifier::Alt));
    assert_eq!(Modifier::from_key(Key::RightAlt), Some(Modifier::Alt));
    assert_eq!(Modifier::from_key(Key::LeftSuper), Some(Modifier::Super));
    assert_eq!(Modifier::from_key(Key::RightSuper), Some(Modifier::Super));
}
