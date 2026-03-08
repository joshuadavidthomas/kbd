//! Integration tests that exercise kbd-iced's public API as an outside consumer would.
//!
//! These complement the unit tests in lib.rs by verifying that:
//! - Import paths work as documented
//! - Converted types compose correctly with kbd's dispatcher
//! - Real workflows (convert event → register → process → match) hold together

use iced_core::keyboard::Event;
use iced_core::keyboard::Location;
use iced_core::keyboard::Modifiers;
use iced_core::keyboard::key;
use kbd::action::Action;
use kbd::dispatcher::Dispatcher;
use kbd::dispatcher::MatchResult;
use kbd::hotkey::Hotkey;
use kbd::hotkey::Modifier;
use kbd::key::Key;
use kbd::key_state::KeyTransition;
use kbd_iced::IcedEventExt;
use kbd_iced::IcedKeyExt;
use kbd_iced::IcedModifiersExt;

fn key_pressed(code: key::Code, modifiers: Modifiers) -> Event {
    Event::KeyPressed {
        key: iced_core::keyboard::Key::Unidentified,
        modified_key: iced_core::keyboard::Key::Unidentified,
        physical_key: key::Physical::Code(code),
        location: Location::Standard,
        modifiers,
        text: None,
        repeat: false,
    }
}

#[test]
fn convert_and_dispatch_simple_hotkey() {
    let mut dispatcher = Dispatcher::new();
    dispatcher
        .register(
            Hotkey::new(Key::S).modifier(Modifier::Ctrl),
            Action::Suppress,
        )
        .unwrap();

    let event = key_pressed(key::Code::KeyS, Modifiers::CTRL);
    let hotkey = event.to_hotkey().unwrap();
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

    let event = key_pressed(key::Code::KeyA, Modifiers::CTRL | Modifiers::SHIFT);
    let hotkey = event.to_hotkey().unwrap();
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

    let event = key_pressed(key::Code::KeyQ, Modifiers::CTRL);
    let hotkey = event.to_hotkey().unwrap();
    let result = dispatcher.process(hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::NoMatch));
}

#[test]
fn unidentified_key_event_returns_none() {
    let event = Event::KeyPressed {
        key: iced_core::keyboard::Key::Unidentified,
        modified_key: iced_core::keyboard::Key::Unidentified,
        physical_key: key::Physical::Unidentified(key::NativeCode::Unidentified),
        location: Location::Standard,
        modifiers: Modifiers::empty(),
        text: None,
        repeat: false,
    };
    assert!(event.to_hotkey().is_none());
}

#[test]
fn modifiers_changed_event_returns_none() {
    let event = Event::ModifiersChanged(Modifiers::CTRL);
    assert!(event.to_hotkey().is_none());
}

#[test]
fn converted_hotkey_equals_manually_built() {
    let event = key_pressed(key::Code::KeyC, Modifiers::CTRL | Modifiers::ALT);
    let from_event = event.to_hotkey().unwrap();
    let manual = Hotkey::new(Key::C)
        .modifier(Modifier::Ctrl)
        .modifier(Modifier::Alt);
    assert_eq!(from_event, manual);
}

#[test]
fn trait_methods_are_independently_composable() {
    let key = key::Code::KeyX.to_key().unwrap();
    let mods = (Modifiers::CTRL | Modifiers::SHIFT).to_modifiers();
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

    let event = key_pressed(key::Code::F5, Modifiers::CTRL | Modifiers::SHIFT);
    let hotkey = event.to_hotkey().unwrap();
    let result = dispatcher.process(hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::Matched { .. }));
}

#[test]
fn string_parsed_and_converted_hotkeys_match() {
    let parsed: Hotkey = "Ctrl+Shift+A".parse().unwrap();

    let event = key_pressed(key::Code::KeyA, Modifiers::CTRL | Modifiers::SHIFT);
    let converted = event.to_hotkey().unwrap();

    assert_eq!(parsed, converted);
}

#[test]
fn key_release_also_converts() {
    let event = Event::KeyReleased {
        key: iced_core::keyboard::Key::Unidentified,
        modified_key: iced_core::keyboard::Key::Unidentified,
        physical_key: key::Physical::Code(key::Code::KeyC),
        location: Location::Standard,
        modifiers: Modifiers::CTRL,
    };
    let hotkey = event.to_hotkey().unwrap();
    assert_eq!(hotkey, Hotkey::new(Key::C).modifier(Modifier::Ctrl));
}

#[test]
fn modifier_key_strips_self_from_modifiers() {
    // iced includes SHIFT in modifiers when ShiftLeft is pressed.
    // The conversion should strip the self-modifier so the hotkey is
    // just ShiftLeft, not Shift+ShiftLeft.
    let event = key_pressed(key::Code::ShiftLeft, Modifiers::SHIFT);
    let hotkey = event.to_hotkey().unwrap();
    assert_eq!(hotkey, Hotkey::new(Key::SHIFT_LEFT));
}

#[test]
fn modifier_key_keeps_other_modifiers() {
    // Pressing ControlLeft while Shift is already held
    let event = key_pressed(key::Code::ControlLeft, Modifiers::SHIFT | Modifiers::CTRL);
    let hotkey = event.to_hotkey().unwrap();
    assert_eq!(
        hotkey,
        Hotkey::new(Key::CONTROL_LEFT).modifier(Modifier::Shift)
    );
}

#[test]
fn logo_maps_to_super_in_dispatch() {
    let mut dispatcher = Dispatcher::new();
    dispatcher
        .register(
            Hotkey::new(Key::S).modifier(Modifier::Super),
            Action::Suppress,
        )
        .unwrap();

    let event = key_pressed(key::Code::KeyS, Modifiers::LOGO);
    let hotkey = event.to_hotkey().unwrap();
    let result = dispatcher.process(hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::Matched { .. }));
}

#[test]
fn media_key_converts_and_dispatches() {
    let mut dispatcher = Dispatcher::new();
    dispatcher
        .register(Hotkey::new(Key::MEDIA_PLAY_PAUSE), Action::Suppress)
        .unwrap();

    let event = key_pressed(key::Code::MediaPlayPause, Modifiers::empty());
    let hotkey = event.to_hotkey().unwrap();
    let result = dispatcher.process(hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::Matched { .. }));
}

#[test]
fn physical_code_wrapper_converts() {
    let physical = key::Physical::Code(key::Code::KeyA);
    assert_eq!(physical.to_key(), Some(Key::A));
}

#[test]
fn physical_unidentified_returns_none() {
    let physical = key::Physical::Unidentified(key::NativeCode::Unidentified);
    assert_eq!(physical.to_key(), None);
}
