#![allow(missing_docs)]
use kbd_global::Error;
use kbd_global::Hotkey;
use kbd_global::HotkeyManager;
use kbd_global::Key;
use kbd_global::Modifier;

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
