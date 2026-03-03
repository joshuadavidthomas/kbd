//! Integration tests that exercise kbd-evdev's public API as an outside consumer would.
//!
//! These complement the unit tests in the source modules by verifying that:
//! - Import paths work as documented
//! - Converted types compose correctly with kbd's dispatcher
//! - Key conversion round-trips hold for real workflows
//!
//! Note: tests that require `/dev/input/` or `/dev/uinput` are not included
//! here because they need hardware access and root/group permissions.

use evdev::KeyCode;
use kbd::prelude::*;
use kbd_evdev::EvdevKeyCodeExt;
use kbd_evdev::KbdKeyExt;

#[test]
fn convert_and_dispatch_simple_hotkey() {
    let mut dispatcher = Dispatcher::new();
    dispatcher
        .register(
            Hotkey::new(Key::A).modifier(Modifier::Ctrl),
            Action::Suppress,
        )
        .unwrap();

    let key = KeyCode::KEY_A.to_key();
    let hotkey = Hotkey::new(key).modifier(Modifier::Ctrl);
    let result = dispatcher.process(&hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::Matched { .. }));
}

#[test]
fn convert_and_dispatch_multi_modifier_hotkey() {
    let mut dispatcher = Dispatcher::new();
    dispatcher
        .register(
            Hotkey::new(Key::S)
                .modifier(Modifier::Ctrl)
                .modifier(Modifier::Shift),
            Action::Suppress,
        )
        .unwrap();

    let key = KeyCode::KEY_S.to_key();
    let hotkey = Hotkey::new(key)
        .modifier(Modifier::Ctrl)
        .modifier(Modifier::Shift);
    let result = dispatcher.process(&hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::Matched { .. }));
}

#[test]
fn unregistered_hotkey_does_not_match() {
    let mut dispatcher = Dispatcher::new();
    dispatcher
        .register(
            Hotkey::new(Key::S).modifier(Modifier::Ctrl),
            Action::Suppress,
        )
        .unwrap();

    let key = KeyCode::KEY_Q.to_key();
    let hotkey = Hotkey::new(key).modifier(Modifier::Ctrl);
    let result = dispatcher.process(&hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::NoMatch));
}

#[test]
fn unmapped_keycode_produces_unidentified() {
    let key = KeyCode::KEY_PROG2.to_key();
    assert_eq!(key, Key::UNIDENTIFIED);
}

#[test]
fn unidentified_key_maps_to_key_unknown() {
    let code = Key::UNIDENTIFIED.to_key_code();
    assert_eq!(code, KeyCode::KEY_UNKNOWN);
}

#[test]
fn converted_hotkey_equals_manually_built() {
    let key = KeyCode::KEY_C.to_key();
    let from_evdev = Hotkey::new(key)
        .modifier(Modifier::Ctrl)
        .modifier(Modifier::Alt);
    let manual = Hotkey::new(Key::C)
        .modifier(Modifier::Ctrl)
        .modifier(Modifier::Alt);
    assert_eq!(from_evdev, manual);
}

#[test]
fn evdev_to_kbd_to_evdev_round_trip() {
    let codes = [
        KeyCode::KEY_A,
        KeyCode::KEY_Z,
        KeyCode::KEY_0,
        KeyCode::KEY_9,
        KeyCode::KEY_F1,
        KeyCode::KEY_F24,
        KeyCode::KEY_ENTER,
        KeyCode::KEY_ESC,
        KeyCode::KEY_SPACE,
        KeyCode::KEY_LEFTCTRL,
        KeyCode::KEY_RIGHTMETA,
        KeyCode::KEY_VOLUMEUP,
        KeyCode::KEY_PLAYPAUSE,
        KeyCode::KEY_BACK,
        KeyCode::KEY_KPENTER,
    ];
    for code in codes {
        let key = code.to_key();
        assert_ne!(key, Key::UNIDENTIFIED, "should map {code:?} to a key");
        let back = key.to_key_code();
        assert_eq!(back, code, "round-trip failed for {code:?}");
    }
}

#[test]
fn kbd_to_evdev_to_kbd_round_trip() {
    let keys = [
        Key::A,
        Key::Z,
        Key::DIGIT0,
        Key::DIGIT9,
        Key::F1,
        Key::F24,
        Key::ENTER,
        Key::ESCAPE,
        Key::SPACE,
        Key::CONTROL_LEFT,
        Key::META_RIGHT,
        Key::AUDIO_VOLUME_UP,
        Key::MEDIA_PLAY_PAUSE,
        Key::BROWSER_BACK,
        Key::NUMPAD_ENTER,
    ];
    for key in keys {
        let code = key.to_key_code();
        assert_ne!(code, KeyCode::KEY_UNKNOWN, "should map {key:?} to a code");
        let back = code.to_key();
        assert_eq!(back, key, "round-trip failed for {key:?}");
    }
}

#[test]
fn function_key_with_modifiers_dispatches() {
    let mut dispatcher = Dispatcher::new();
    dispatcher
        .register(
            Hotkey::new(Key::F5)
                .modifier(Modifier::Ctrl)
                .modifier(Modifier::Shift),
            Action::Suppress,
        )
        .unwrap();

    let key = KeyCode::KEY_F5.to_key();
    let hotkey = Hotkey::new(key)
        .modifier(Modifier::Ctrl)
        .modifier(Modifier::Shift);
    let result = dispatcher.process(&hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::Matched { .. }));
}

#[test]
fn string_parsed_and_converted_hotkeys_match() {
    let parsed: Hotkey = "Ctrl+Shift+A".parse().unwrap();

    let key = KeyCode::KEY_A.to_key();
    let converted = Hotkey::new(key)
        .modifier(Modifier::Ctrl)
        .modifier(Modifier::Shift);

    assert_eq!(parsed, converted);
}

#[test]
fn media_key_converts_and_dispatches() {
    let mut dispatcher = Dispatcher::new();
    dispatcher
        .register(Hotkey::new(Key::MEDIA_PLAY_PAUSE), Action::Suppress)
        .unwrap();

    let key = KeyCode::KEY_PLAYPAUSE.to_key();
    let hotkey = Hotkey::new(key);
    let result = dispatcher.process(&hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::Matched { .. }));
}

#[test]
fn browser_key_converts_and_dispatches() {
    let mut dispatcher = Dispatcher::new();
    dispatcher
        .register(Hotkey::new(Key::BROWSER_BACK), Action::Suppress)
        .unwrap();

    let key = KeyCode::KEY_BACK.to_key();
    let hotkey = Hotkey::new(key);
    let result = dispatcher.process(&hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::Matched { .. }));
}
