//! Integration tests that exercise kbd-crossterm's public API as an outside consumer would.
//!
//! These complement the unit tests in lib.rs by verifying that:
//! - Import paths work as documented
//! - Converted types compose correctly with kbd's dispatcher
//! - Real workflows (convert event → register → process → match) hold together

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use crossterm::event::ModifierKeyCode;
use kbd::action::Action;
use kbd::dispatcher::Dispatcher;
use kbd::dispatcher::MatchResult;
use kbd::hotkey::Hotkey;
use kbd::hotkey::Modifier;
use kbd::key::Key;
use kbd::key_state::KeyTransition;
use kbd_crossterm::CrosstermEventExt;
use kbd_crossterm::CrosstermKeyExt;
use kbd_crossterm::CrosstermModifiersExt;

#[test]
fn convert_and_dispatch_simple_hotkey() {
    let mut dispatcher = Dispatcher::new();
    dispatcher
        .register(
            Hotkey::new(Key::S).modifier(Modifier::Ctrl),
            Action::Suppress,
        )
        .unwrap();

    let event = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL);
    let hotkey = event.to_hotkey().unwrap();
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

    // crossterm reports uppercase 'A' with both modifiers
    let event = KeyEvent::new(
        KeyCode::Char('A'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    );
    let hotkey = event.to_hotkey().unwrap();
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

    let event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
    let hotkey = event.to_hotkey().unwrap();
    let result = dispatcher.process(&hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::NoMatch));
}

#[test]
fn unmappable_event_skipped_gracefully() {
    let event = KeyEvent::new(KeyCode::Null, KeyModifiers::NONE);
    assert!(event.to_hotkey().is_none());
}

#[test]
fn modifier_key_strips_self_modifier() {
    // crossterm reports LeftShift with SHIFT in the modifier bitflags
    let event = KeyEvent::new(
        KeyCode::Modifier(ModifierKeyCode::LeftShift),
        KeyModifiers::SHIFT,
    );
    let hotkey = event.to_hotkey().unwrap();

    // Self-modifier should be stripped — the hotkey should be just ShiftLeft,
    // not Shift+ShiftLeft
    assert_eq!(hotkey, Hotkey::new(Key::SHIFT_LEFT));
}

#[test]
fn modifier_key_preserves_other_modifiers() {
    // Pressing LeftControl while Shift is already held
    let event = KeyEvent::new(
        KeyCode::Modifier(ModifierKeyCode::LeftControl),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    );
    let hotkey = event.to_hotkey().unwrap();

    // CONTROL stripped (it's the key itself), but SHIFT preserved
    assert_eq!(
        hotkey,
        Hotkey::new(Key::CONTROL_LEFT).modifier(Modifier::Shift)
    );
}

#[test]
fn converted_hotkey_equals_manually_built() {
    let event = KeyEvent::new(
        KeyCode::Char('c'),
        KeyModifiers::CONTROL | KeyModifiers::ALT,
    );
    let from_event = event.to_hotkey().unwrap();
    let manual = Hotkey::new(Key::C)
        .modifier(Modifier::Ctrl)
        .modifier(Modifier::Alt);
    assert_eq!(from_event, manual);
}

#[test]
fn trait_methods_are_independently_composable() {
    // Verify the traits can be used separately and composed manually
    let key = KeyCode::Char('x').to_key().unwrap();
    let mods = (KeyModifiers::CONTROL | KeyModifiers::SHIFT).to_modifiers();
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

    let event = KeyEvent::new(KeyCode::F(5), KeyModifiers::CONTROL | KeyModifiers::SHIFT);
    let hotkey = event.to_hotkey().unwrap();
    let result = dispatcher.process(&hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::Matched { .. }));
}

#[test]
fn string_parsed_and_converted_hotkeys_match() {
    let parsed: Hotkey = "Ctrl+Shift+A".parse().unwrap();

    let event = KeyEvent::new(
        KeyCode::Char('A'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    );
    let converted = event.to_hotkey().unwrap();

    assert_eq!(parsed, converted);
}
