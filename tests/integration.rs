use evdev::KeyCode;
use evdev_hotkey::{Error, HotkeyManager};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

fn create_manager_or_skip() -> Option<HotkeyManager> {
    match HotkeyManager::new() {
        Ok(manager) => Some(manager),
        Err(Error::PermissionDenied(_))
        | Err(Error::NoKeyboardsFound)
        | Err(Error::DeviceAccess(_)) => {
            println!("Skipping test: environment has no accessible input devices");
            None
        }
        Err(err) => panic!("Unexpected manager creation error: {}", err),
    }
}

#[test]
fn test_manager_creation() {
    match HotkeyManager::new() {
        Ok(_) => println!("Manager created successfully"),
        Err(Error::PermissionDenied(_))
        | Err(Error::NoKeyboardsFound)
        | Err(Error::DeviceAccess(_)) => {
            println!("Manager creation skipped: environment has no accessible input devices")
        }
        Err(err) => panic!("Unexpected manager creation error: {}", err),
    }
}

#[test]
fn test_register_hotkey() {
    let manager = match create_manager_or_skip() {
        Some(m) => m,
        None => return,
    };

    let triggered = Arc::new(AtomicBool::new(false));
    let triggered_clone = triggered.clone();

    let _handle = manager
        .register(
            KeyCode::KEY_A,
            &[KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTSHIFT],
            move || {
                triggered_clone.store(true, Ordering::SeqCst);
            },
        )
        .unwrap();

    // Note: Cannot actually test hotkey triggering in automated tests
    // This just verifies registration doesn't crash
    println!("Hotkey registered successfully");
}

#[test]
fn test_register_multiple_hotkeys() {
    let manager = match create_manager_or_skip() {
        Some(m) => m,
        None => return,
    };

    let _handle1 = manager
        .register(
            KeyCode::KEY_A,
            &[KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTSHIFT],
            || {
                println!("Action A!");
            },
        )
        .unwrap();

    let _handle2 = manager
        .register(
            KeyCode::KEY_B,
            &[KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTSHIFT],
            || {
                println!("Action B!");
            },
        )
        .unwrap();

    // Same key with different modifiers
    let _handle3 = manager
        .register(
            KeyCode::KEY_C,
            &[KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTALT],
            || {
                println!("Action C (Ctrl+Alt)!");
            },
        )
        .unwrap();

    let _handle4 = manager
        .register(
            KeyCode::KEY_C,
            &[KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTSHIFT],
            || {
                println!("Action C2 (Ctrl+Shift)!");
            },
        )
        .unwrap();

    println!("Multiple hotkeys registered successfully");
}

#[test]
fn test_unregister_hotkey() {
    let manager = match create_manager_or_skip() {
        Some(m) => m,
        None => return,
    };

    let handle = manager
        .register(KeyCode::KEY_A, &[KeyCode::KEY_LEFTCTRL], || {
            println!("Triggered!");
        })
        .unwrap();

    // Unregister should not panic
    handle.unregister().unwrap();
    println!("Hotkey unregistered successfully");
}

#[test]
fn test_handle_unregister_cleans_up() {
    let manager = match create_manager_or_skip() {
        Some(m) => m,
        None => return,
    };

    let triggered = Arc::new(AtomicBool::new(false));
    let triggered_clone = triggered.clone();

    let handle = manager
        .register(KeyCode::KEY_X, &[KeyCode::KEY_LEFTCTRL], move || {
            triggered_clone.store(true, Ordering::SeqCst);
        })
        .unwrap();

    // Give the listener thread a moment to start
    std::thread::sleep(Duration::from_millis(100));

    // Unregister
    handle.unregister().unwrap();

    // After unregister, the callback should not be in the registry anymore
    // We can't test actual triggering, but we verify unregister doesn't crash
    println!("Handle cleanup verified");
}
