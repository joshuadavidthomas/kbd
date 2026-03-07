//! Integration tests that exercise kbd-egui's public API as an outside consumer would.
//!
//! These complement the unit tests in lib.rs by verifying that:
//! - Import paths work as documented
//! - Converted types compose correctly with kbd's dispatcher
//! - Real workflows (convert event → register → process → match) hold together

use egui::Key as EguiKey;
use egui::Modifiers;
use kbd::action::Action;
use kbd::dispatcher::Dispatcher;
use kbd::dispatcher::MatchResult;
use kbd::hotkey::Hotkey;
use kbd::hotkey::Modifier;
use kbd::key::Key;
use kbd::key_state::KeyTransition;
use kbd_egui::EguiEventExt;
use kbd_egui::EguiKeyExt;
use kbd_egui::EguiModifiersExt;

fn key_event(key: EguiKey, modifiers: Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
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

    let event = key_event(EguiKey::S, Modifiers::CTRL);
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

    let mods = Modifiers {
        alt: false,
        ctrl: true,
        shift: true,
        mac_cmd: false,
        command: false,
    };
    let event = key_event(EguiKey::A, mods);
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

    let event = key_event(EguiKey::Q, Modifiers::CTRL);
    let hotkey = event.to_hotkey().unwrap();
    let result = dispatcher.process(hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::NoMatch));
}

#[test]
fn unmappable_key_event_returns_none() {
    let event = key_event(EguiKey::Colon, Modifiers::NONE);
    assert!(event.to_hotkey().is_none());
}

#[test]
fn non_key_event_returns_none() {
    let event = egui::Event::PointerMoved(egui::pos2(10.0, 20.0));
    assert!(event.to_hotkey().is_none());
}

#[test]
fn converted_hotkey_equals_manually_built() {
    let event = key_event(
        EguiKey::C,
        Modifiers {
            alt: true,
            ctrl: true,
            shift: false,
            mac_cmd: false,
            command: false,
        },
    );
    let from_event = event.to_hotkey().unwrap();
    let manual = Hotkey::new(Key::C)
        .modifier(Modifier::Ctrl)
        .modifier(Modifier::Alt);
    assert_eq!(from_event, manual);
}

#[test]
fn trait_methods_are_independently_composable() {
    let key = EguiKey::X.to_key().unwrap();
    let mods = Modifiers {
        alt: false,
        ctrl: true,
        shift: true,
        mac_cmd: false,
        command: false,
    }
    .to_modifiers();
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

    let mods = Modifiers {
        alt: false,
        ctrl: true,
        shift: true,
        mac_cmd: false,
        command: false,
    };
    let event = key_event(EguiKey::F5, mods);
    let hotkey = event.to_hotkey().unwrap();
    let result = dispatcher.process(hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::Matched { .. }));
}

#[test]
fn string_parsed_and_converted_hotkeys_match() {
    let parsed: Hotkey = "Ctrl+Shift+A".parse().unwrap();

    let mods = Modifiers {
        alt: false,
        ctrl: true,
        shift: true,
        mac_cmd: false,
        command: false,
    };
    let event = key_event(EguiKey::A, mods);
    let converted = event.to_hotkey().unwrap();

    assert_eq!(parsed, converted);
}

#[test]
fn mac_cmd_maps_to_super_in_dispatch() {
    let mut dispatcher = Dispatcher::new();
    dispatcher
        .register(
            Hotkey::new(Key::S).modifier(Modifier::Super),
            Action::Suppress,
        )
        .unwrap();

    let mods = Modifiers {
        alt: false,
        ctrl: false,
        shift: false,
        mac_cmd: true,
        command: true,
    };
    let event = key_event(EguiKey::S, mods);
    let hotkey = event.to_hotkey().unwrap();
    let result = dispatcher.process(hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::Matched { .. }));
}

#[test]
fn browser_back_converts_and_dispatches() {
    let mut dispatcher = Dispatcher::new();
    dispatcher
        .register(Hotkey::new(Key::BROWSER_BACK), Action::Suppress)
        .unwrap();

    let event = key_event(EguiKey::BrowserBack, Modifiers::NONE);
    let hotkey = event.to_hotkey().unwrap();
    let result = dispatcher.process(hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::Matched { .. }));
}
