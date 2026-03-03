#![allow(missing_docs)]
use kbd_evdev::EvdevKeyCodeExt;
use kbd_evdev::KbdKeyExt;
use kbd_global::Key;
use kbd_global::Modifier;

#[test]
fn key_round_trips_through_evdev_keycode() {
    for key in [
        Key::A,
        Key::DIGIT9,
        Key::F24,
        Key::ENTER,
        Key::ARROW_LEFT,
        Key::NUMPAD_ENTER,
        Key::CONTROL_LEFT,
        Key::META_RIGHT,
    ] {
        let code = key.to_key_code();
        let reparsed = code.to_key();
        assert_eq!(reparsed, key, "failed for {key:?}");
    }
}

#[test]
fn media_key_round_trips_through_evdev() {
    use evdev::KeyCode;
    assert_eq!(KeyCode::KEY_VOLUMEUP.to_key(), Key::AUDIO_VOLUME_UP);
    assert_eq!(Key::AUDIO_VOLUME_UP.to_key_code(), KeyCode::KEY_VOLUMEUP);
}

#[test]
fn unmapped_evdev_keycode_maps_to_unknown_key() {
    use evdev::KeyCode;
    assert_eq!(KeyCode::KEY_PROG2.to_key(), Key::UNIDENTIFIED);
}

#[test]
fn modifier_try_from_key_canonicalizes_left_and_right_variants() {
    assert_eq!(Modifier::try_from(Key::CONTROL_LEFT), Ok(Modifier::Ctrl));
    assert_eq!(Modifier::try_from(Key::CONTROL_RIGHT), Ok(Modifier::Ctrl));
    assert_eq!(Modifier::try_from(Key::SHIFT_LEFT), Ok(Modifier::Shift));
    assert_eq!(Modifier::try_from(Key::SHIFT_RIGHT), Ok(Modifier::Shift));
    assert_eq!(Modifier::try_from(Key::ALT_LEFT), Ok(Modifier::Alt));
    assert_eq!(Modifier::try_from(Key::ALT_RIGHT), Ok(Modifier::Alt));
    assert_eq!(Modifier::try_from(Key::META_LEFT), Ok(Modifier::Super));
    assert_eq!(Modifier::try_from(Key::META_RIGHT), Ok(Modifier::Super));
    assert_eq!(Modifier::try_from(Key::A), Err(Key::A));
}
