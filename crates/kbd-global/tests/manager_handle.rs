#![allow(missing_docs)]
use kbd::hotkey::Hotkey;
use kbd::hotkey::Modifier;
use kbd::key::Key;
use kbd_global::Error;
use kbd_global::HotkeyManager;

#[test]
fn register_and_drop_handle_unregisters_hotkey() {
    let manager = HotkeyManager::new().expect("manager should initialize");
    let hotkey = Hotkey::new(Key::C).modifier(Modifier::Ctrl);

    let handle = manager
        .register(hotkey.clone(), || {})
        .expect("register should succeed");

    assert!(
        manager
            .is_registered(hotkey.clone())
            .expect("query should succeed")
    );

    drop(handle);

    // Command channel is FIFO: the Unregister from drop is enqueued before
    // this IsRegistered query, so drain_commands() processes them in order.
    assert!(!manager.is_registered(hotkey).expect("query should succeed"));
}

#[test]
fn duplicate_hotkey_registration_returns_conflict_error() {
    let manager = HotkeyManager::new().expect("manager should initialize");
    let hotkey = Hotkey::new(Key::A).modifier(Modifier::Ctrl);

    let _first = manager
        .register(hotkey.clone(), || {})
        .expect("first registration should succeed");

    let duplicate = manager.register(hotkey, || {});
    assert!(matches!(duplicate, Err(Error::AlreadyRegistered)));
}

#[test]
fn is_key_pressed_returns_false_when_no_keys_pressed() {
    let manager = HotkeyManager::new().expect("manager should initialize");

    assert!(
        !manager
            .is_key_pressed(Key::A)
            .expect("query should succeed")
    );
}

#[test]
fn active_modifiers_returns_empty_when_no_keys_pressed() {
    let manager = HotkeyManager::new().expect("manager should initialize");

    let modifiers = manager.active_modifiers().expect("query should succeed");
    assert!(modifiers.is_empty());
}

#[test]
fn shutdown_stops_handle_unregistration_commands() {
    let manager = HotkeyManager::new().expect("manager should initialize");

    let handle = manager
        .register(Hotkey::new(Key::B).modifier(Modifier::Alt), || {})
        .expect("register should succeed");

    manager.shutdown().expect("shutdown should succeed");

    let unregister = handle.unregister();
    assert!(matches!(unregister, Err(Error::ManagerStopped)));
}

#[test]
fn register_sequence_returns_guard() {
    let manager = HotkeyManager::new().expect("manager should initialize");

    let sequence = kbd::hotkey::HotkeySequence::new(vec![
        Hotkey::new(Key::K).modifier(Modifier::Ctrl),
        Hotkey::new(Key::C).modifier(Modifier::Ctrl),
    ])
    .unwrap();

    let guard = manager
        .register_sequence(sequence, || {})
        .expect("sequence registration should succeed");

    drop(guard);
}

#[test]
fn register_sequence_accepts_string_input() {
    let manager = HotkeyManager::new().expect("manager should initialize");

    let guard = manager
        .register_sequence("Ctrl+K, Ctrl+C", || {})
        .expect("sequence string registration should succeed");

    drop(guard);
}

#[test]
fn register_sequence_accepts_vec_hotkeys_input() {
    let manager = HotkeyManager::new().expect("manager should initialize");

    let guard = manager
        .register_sequence(
            vec![
                Hotkey::new(Key::K).modifier(Modifier::Ctrl),
                Hotkey::new(Key::C).modifier(Modifier::Ctrl),
            ],
            || {},
        )
        .expect("sequence vec registration should succeed");

    drop(guard);
}

#[test]
fn register_sequence_reports_parse_error_for_string_input() {
    let manager = HotkeyManager::new().expect("manager should initialize");

    let result = manager.register_sequence("Ctrl+K, Ctrl+Nope", || {});
    assert!(matches!(result, Err(Error::Parse(_))));
}

#[test]
fn register_sequence_str_returns_guard() {
    let manager = HotkeyManager::new().expect("manager should initialize");

    let guard = manager
        .register_sequence_str("Ctrl+K, Ctrl+C", || {})
        .expect("sequence string registration should succeed");

    drop(guard);
}

#[test]
fn register_sequence_str_reports_parse_error() {
    let manager = HotkeyManager::new().expect("manager should initialize");

    let result = manager.register_sequence_str("Ctrl+K, Ctrl+Nope", || {});
    assert!(matches!(result, Err(Error::Parse(_))));
}

#[test]
fn pending_sequence_is_none_when_idle() {
    let manager = HotkeyManager::new().expect("manager should initialize");

    let pending = manager
        .pending_sequence()
        .expect("pending query should succeed");
    assert!(pending.is_none());
}
