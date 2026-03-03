//! Integration tests that exercise kbd-winit's public API as an outside consumer would.
//!
//! These complement the unit tests in lib.rs by verifying that:
//! - Import paths work as documented
//! - Converted types compose correctly with kbd's dispatcher
//! - Real workflows (convert → register → process → match) hold together
//!
//! Note: winit's `KeyEvent` has private fields that prevent direct
//! construction outside the winit crate. These tests exercise the public
//! `winit_key_to_hotkey` function and the individual extension traits
//! instead. The `WinitEventExt` trait is tested via unit tests in lib.rs
//! where we can validate the delegation.

use kbd::prelude::*;
use kbd_winit::WinitKeyExt;
use kbd_winit::WinitModifiersExt;
use kbd_winit::winit_key_to_hotkey;
use winit::keyboard::KeyCode;
use winit::keyboard::ModifiersState;
use winit::keyboard::PhysicalKey;

#[test]
fn convert_and_dispatch_simple_hotkey() {
    let mut dispatcher = Dispatcher::new();
    dispatcher
        .register(
            Hotkey::new(Key::S).modifier(Modifier::Ctrl),
            Action::Suppress,
        )
        .unwrap();

    let hotkey =
        winit_key_to_hotkey(PhysicalKey::Code(KeyCode::KeyS), ModifiersState::CONTROL).unwrap();
    let result = dispatcher.process(&hotkey, KeyTransition::Press);
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
    let hotkey = winit_key_to_hotkey(PhysicalKey::Code(KeyCode::KeyA), mods).unwrap();
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

    let hotkey =
        winit_key_to_hotkey(PhysicalKey::Code(KeyCode::KeyQ), ModifiersState::CONTROL).unwrap();
    let result = dispatcher.process(&hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::NoMatch));
}

#[test]
fn unidentified_key_returns_none() {
    let hotkey = winit_key_to_hotkey(
        PhysicalKey::Unidentified(winit::keyboard::NativeKeyCode::Unidentified),
        ModifiersState::empty(),
    );
    assert!(hotkey.is_none());
}

#[test]
fn converted_hotkey_equals_manually_built() {
    let mods = ModifiersState::CONTROL | ModifiersState::ALT;
    let from_fn = winit_key_to_hotkey(PhysicalKey::Code(KeyCode::KeyC), mods).unwrap();
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
    let hotkey = winit_key_to_hotkey(PhysicalKey::Code(KeyCode::F5), mods).unwrap();
    let result = dispatcher.process(&hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::Matched { .. }));
}

#[test]
fn string_parsed_and_converted_hotkeys_match() {
    let parsed: Hotkey = "Ctrl+Shift+A".parse().unwrap();

    let mods = ModifiersState::CONTROL | ModifiersState::SHIFT;
    let converted = winit_key_to_hotkey(PhysicalKey::Code(KeyCode::KeyA), mods).unwrap();

    assert_eq!(parsed, converted);
}

#[test]
fn modifier_key_strips_self_from_modifiers() {
    // winit includes SHIFT in ModifiersState when ShiftLeft is pressed.
    // The conversion should strip the self-modifier so the hotkey is
    // just ShiftLeft, not Shift+ShiftLeft.
    let hotkey =
        winit_key_to_hotkey(PhysicalKey::Code(KeyCode::ShiftLeft), ModifiersState::SHIFT).unwrap();
    assert_eq!(hotkey, Hotkey::new(Key::SHIFT_LEFT));
}

#[test]
fn modifier_key_keeps_other_modifiers() {
    // Pressing ControlLeft while Shift is already held
    let mods = ModifiersState::SHIFT | ModifiersState::CONTROL;
    let hotkey = winit_key_to_hotkey(PhysicalKey::Code(KeyCode::ControlLeft), mods).unwrap();
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

    let hotkey =
        winit_key_to_hotkey(PhysicalKey::Code(KeyCode::KeyS), ModifiersState::SUPER).unwrap();
    let result = dispatcher.process(&hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::Matched { .. }));
}

#[test]
fn media_key_converts_and_dispatches() {
    let mut dispatcher = Dispatcher::new();
    dispatcher
        .register(Hotkey::new(Key::MEDIA_PLAY_PAUSE), Action::Suppress)
        .unwrap();

    let hotkey = winit_key_to_hotkey(
        PhysicalKey::Code(KeyCode::MediaPlayPause),
        ModifiersState::empty(),
    )
    .unwrap();
    let result = dispatcher.process(&hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::Matched { .. }));
}

#[test]
fn super_left_right_maps_to_meta() {
    // winit's SuperLeft/SuperRight → kbd's META_LEFT/META_RIGHT
    assert_eq!(KeyCode::SuperLeft.to_key(), Some(Key::META_LEFT));
    assert_eq!(KeyCode::SuperRight.to_key(), Some(Key::META_RIGHT));
}

#[test]
fn super_key_strips_self() {
    // Pressing SuperLeft — winit reports SUPER in ModifiersState.
    let hotkey =
        winit_key_to_hotkey(PhysicalKey::Code(KeyCode::SuperLeft), ModifiersState::SUPER).unwrap();
    assert_eq!(hotkey, Hotkey::new(Key::META_LEFT));
}

#[test]
fn physical_key_delegates_to_keycode() {
    // PhysicalKey::Code delegates to KeyCode's implementation
    let from_keycode = KeyCode::KeyA.to_key();
    let from_physical = PhysicalKey::Code(KeyCode::KeyA).to_key();
    assert_eq!(from_keycode, from_physical);
}

#[test]
fn meta_legacy_alias_maps_to_meta_left() {
    // winit's Meta (no left/right distinction) maps to META_LEFT
    assert_eq!(KeyCode::Meta.to_key(), Some(Key::META_LEFT));
}
