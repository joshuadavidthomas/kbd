//! Integration tests that exercise kbd-tao's public API as an outside consumer would.
//!
//! These complement the unit tests in lib.rs by verifying that:
//! - Import paths work as documented
//! - Converted types compose correctly with kbd's dispatcher
//! - Real workflows (convert → register → process → match) hold together
//!
//! Note: tao's `KeyEvent` has a `pub(crate)` field (`platform_specific`)
//! that prevents direct construction outside the tao crate. These tests
//! exercise the public `tao_key_to_hotkey` function and the individual
//! extension traits instead. The `TaoEventExt` trait is tested via unit
//! tests in lib.rs where we can validate the delegation.

use kbd::action::Action;
use kbd::dispatcher::Dispatcher;
use kbd::dispatcher::MatchResult;
use kbd::hotkey::Hotkey;
use kbd::hotkey::Modifier;
use kbd::key::Key;
use kbd::key_state::KeyTransition;
use kbd_tao::TaoKeyExt;
use kbd_tao::TaoModifiersExt;
use kbd_tao::tao_key_to_hotkey;
use tao::keyboard::KeyCode;
use tao::keyboard::ModifiersState;

#[test]
fn convert_and_dispatch_simple_hotkey() {
    let mut dispatcher = Dispatcher::new();
    dispatcher
        .register(
            Hotkey::new(Key::S).modifier(Modifier::Ctrl),
            Action::Suppress,
        )
        .unwrap();

    let hotkey = tao_key_to_hotkey(KeyCode::KeyS, ModifiersState::CONTROL).unwrap();
    let result = dispatcher.process(hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::Matched { .. }));
}

#[test]
fn convert_and_dispatch_multi_modifier_hotkey() {
    let mut dispatcher = Dispatcher::new();
    dispatcher
        .register(
            Hotkey::new(Key::A)
                .modifier(Modifier::Ctrl)
                .modifier(Modifier::Shift),
            Action::Suppress,
        )
        .unwrap();

    let mods = ModifiersState::CONTROL | ModifiersState::SHIFT;
    let hotkey = tao_key_to_hotkey(KeyCode::KeyA, mods).unwrap();
    let result = dispatcher.process(hotkey, KeyTransition::Press);
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

    let hotkey = tao_key_to_hotkey(KeyCode::KeyQ, ModifiersState::CONTROL).unwrap();
    let result = dispatcher.process(hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::NoMatch));
}

#[test]
fn unidentified_key_returns_none() {
    let hotkey = tao_key_to_hotkey(
        KeyCode::Unidentified(tao::keyboard::NativeKeyCode::Unidentified),
        ModifiersState::empty(),
    );
    assert!(hotkey.is_none());
}

#[test]
fn converted_hotkey_equals_manually_built() {
    let mods = ModifiersState::CONTROL | ModifiersState::ALT;
    let from_fn = tao_key_to_hotkey(KeyCode::KeyC, mods).unwrap();
    let manual = Hotkey::new(Key::C)
        .modifier(Modifier::Ctrl)
        .modifier(Modifier::Alt);
    assert_eq!(from_fn, manual);
}

#[test]
fn trait_methods_are_independently_composable() {
    let key = KeyCode::KeyX.to_key().unwrap();
    let mods = (ModifiersState::CONTROL | ModifiersState::SHIFT).to_modifiers();
    let hotkey = Hotkey::with_modifiers(key, mods);

    let expected = Hotkey::new(Key::X)
        .modifier(Modifier::Ctrl)
        .modifier(Modifier::Shift);
    assert_eq!(hotkey, expected);
}

#[test]
fn function_key_with_modifiers_roundtrip() {
    let mut dispatcher = Dispatcher::new();
    dispatcher
        .register(
            Hotkey::new(Key::F5)
                .modifier(Modifier::Ctrl)
                .modifier(Modifier::Shift),
            Action::Suppress,
        )
        .unwrap();

    let mods = ModifiersState::CONTROL | ModifiersState::SHIFT;
    let hotkey = tao_key_to_hotkey(KeyCode::F5, mods).unwrap();
    let result = dispatcher.process(hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::Matched { .. }));
}

#[test]
fn string_parsed_and_converted_hotkeys_match() {
    let parsed: Hotkey = "Ctrl+Shift+A".parse().unwrap();

    let mods = ModifiersState::CONTROL | ModifiersState::SHIFT;
    let converted = tao_key_to_hotkey(KeyCode::KeyA, mods).unwrap();

    assert_eq!(parsed, converted);
}

#[test]
fn modifier_key_strips_self_from_modifiers() {
    // tao includes SHIFT in ModifiersState when ShiftLeft is pressed.
    // The conversion should strip the self-modifier so the hotkey is
    // just ShiftLeft, not Shift+ShiftLeft.
    let hotkey = tao_key_to_hotkey(KeyCode::ShiftLeft, ModifiersState::SHIFT).unwrap();
    assert_eq!(hotkey, Hotkey::new(Key::SHIFT_LEFT));
}

#[test]
fn modifier_key_keeps_other_modifiers() {
    // Pressing ControlLeft while Shift is already held
    let mods = ModifiersState::SHIFT | ModifiersState::CONTROL;
    let hotkey = tao_key_to_hotkey(KeyCode::ControlLeft, mods).unwrap();
    assert_eq!(
        hotkey,
        Hotkey::new(Key::CONTROL_LEFT).modifier(Modifier::Shift)
    );
}

#[test]
fn super_maps_to_super_in_dispatch() {
    let mut dispatcher = Dispatcher::new();
    dispatcher
        .register(
            Hotkey::new(Key::S).modifier(Modifier::Super),
            Action::Suppress,
        )
        .unwrap();

    let hotkey = tao_key_to_hotkey(KeyCode::KeyS, ModifiersState::SUPER).unwrap();
    let result = dispatcher.process(hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::Matched { .. }));
}

#[test]
fn media_key_converts_and_dispatches() {
    let mut dispatcher = Dispatcher::new();
    dispatcher
        .register(Hotkey::new(Key::MEDIA_PLAY_PAUSE), Action::Suppress)
        .unwrap();

    let hotkey = tao_key_to_hotkey(KeyCode::MediaPlayPause, ModifiersState::empty()).unwrap();
    let result = dispatcher.process(hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::Matched { .. }));
}

#[test]
fn super_left_right_maps_to_meta() {
    // tao's SuperLeft/SuperRight → kbd's META_LEFT/META_RIGHT
    assert_eq!(KeyCode::SuperLeft.to_key(), Some(Key::META_LEFT));
    assert_eq!(KeyCode::SuperRight.to_key(), Some(Key::META_RIGHT));
}

#[test]
fn super_key_strips_self() {
    // Pressing SuperLeft — tao reports SUPER in ModifiersState.
    let hotkey = tao_key_to_hotkey(KeyCode::SuperLeft, ModifiersState::SUPER).unwrap();
    assert_eq!(hotkey, Hotkey::new(Key::META_LEFT));
}
