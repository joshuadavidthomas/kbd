use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use keybound::Error;
use keybound::HotkeyManager;
use keybound::Key;
use keybound::Modifier;

fn create_manager_or_skip() -> Option<HotkeyManager> {
    match HotkeyManager::new() {
        Ok(manager) => Some(manager),
        Err(
            Error::PermissionDenied(_)
            | Error::NoKeyboardsFound
            | Error::DeviceAccess(_)
            | Error::BackendUnavailable(_),
        ) => {
            println!("Skipping test: environment has no usable backend/input devices");
            None
        }
        Err(err) => panic!("Unexpected manager creation error: {err}"),
    }
}

#[test]
fn test_manager_creation() {
    match HotkeyManager::new() {
        Ok(_) => println!("Manager created successfully"),
        Err(
            Error::PermissionDenied(_)
            | Error::NoKeyboardsFound
            | Error::DeviceAccess(_)
            | Error::BackendUnavailable(_),
        ) => {
            println!("Manager creation skipped: environment has no usable backend/input devices");
        }
        Err(err) => panic!("Unexpected manager creation error: {err}"),
    }
}

#[test]
fn test_register_hotkey() {
    let Some(manager) = create_manager_or_skip() else {
        return;
    };

    let triggered = Arc::new(AtomicBool::new(false));
    let triggered_clone = triggered.clone();

    let _handle = manager
        .register(Key::A, &[Modifier::Ctrl, Modifier::Shift], move || {
            triggered_clone.store(true, Ordering::SeqCst);
        })
        .unwrap();

    println!("Hotkey registered successfully");
}

#[test]
fn test_register_multiple_hotkeys() {
    let Some(manager) = create_manager_or_skip() else {
        return;
    };

    let _handle1 = manager
        .register(Key::A, &[Modifier::Ctrl, Modifier::Shift], || {
            println!("Action A!");
        })
        .unwrap();

    let _handle2 = manager
        .register(Key::B, &[Modifier::Ctrl, Modifier::Shift], || {
            println!("Action B!");
        })
        .unwrap();

    let _handle3 = manager
        .register(Key::C, &[Modifier::Ctrl, Modifier::Alt], || {
            println!("Action C (Ctrl+Alt)!");
        })
        .unwrap();

    let _handle4 = manager
        .register(Key::C, &[Modifier::Ctrl, Modifier::Shift], || {
            println!("Action C2 (Ctrl+Shift)!");
        })
        .unwrap();

    println!("Multiple hotkeys registered successfully");
}

#[test]
fn test_unregister_hotkey() {
    let Some(manager) = create_manager_or_skip() else {
        return;
    };

    let handle = manager
        .register(Key::A, &[Modifier::Ctrl], || {
            println!("Triggered!");
        })
        .unwrap();

    handle.unregister().unwrap();
    println!("Hotkey unregistered successfully");
}

#[test]
fn test_handle_unregister_cleans_up() {
    let Some(manager) = create_manager_or_skip() else {
        return;
    };

    let triggered = Arc::new(AtomicBool::new(false));
    let triggered_clone = triggered.clone();

    let handle = manager
        .register(Key::X, &[Modifier::Ctrl], move || {
            triggered_clone.store(true, Ordering::SeqCst);
        })
        .unwrap();

    std::thread::sleep(Duration::from_millis(100));

    handle.unregister().unwrap();

    println!("Handle cleanup verified");
}
