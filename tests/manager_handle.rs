use keybound::Error;
use keybound::HotkeyManager;
use keybound::Key;
use keybound::Modifier;

#[test]
fn register_and_drop_handle_unregisters_hotkey() {
    let manager = HotkeyManager::new().expect("manager should initialize");

    let handle = manager
        .register(Key::C, &[Modifier::Ctrl], || {})
        .expect("register should succeed");

    assert!(manager
        .is_registered(Key::C, &[Modifier::Ctrl])
        .expect("query should succeed"));

    drop(handle);

    // Command channel is FIFO: the Unregister from drop is enqueued before
    // this IsRegistered query, so drain_commands() processes them in order.
    assert!(!manager
        .is_registered(Key::C, &[Modifier::Ctrl])
        .expect("query should succeed"));
}

#[test]
fn duplicate_hotkey_registration_returns_conflict_error() {
    let manager = HotkeyManager::new().expect("manager should initialize");

    let _first = manager
        .register(Key::A, &[Modifier::Ctrl], || {})
        .expect("first registration should succeed");

    let duplicate = manager.register(Key::A, &[Modifier::Ctrl], || {});
    assert!(matches!(duplicate, Err(Error::AlreadyRegistered)));
}

#[test]
fn is_key_pressed_returns_false_when_no_keys_pressed() {
    let manager = HotkeyManager::new().expect("manager should initialize");

    assert!(!manager
        .is_key_pressed(Key::A)
        .expect("query should succeed"));
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
        .register(Key::B, &[Modifier::Alt], || {})
        .expect("register should succeed");

    manager.shutdown().expect("shutdown should succeed");

    let unregister = handle.unregister();
    assert!(matches!(unregister, Err(Error::ManagerStopped)));
}
