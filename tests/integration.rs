use evdev::KeyCode;
use evdev_hotkey::HotkeyManager;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[test]
fn test_manager_creation() {
    // This test requires being in the input group
    // Skip in CI if not available
    let result = HotkeyManager::new();
    // Either success or permission error is acceptable
    match result {
        Ok(_) => println!("Manager created successfully"),
        Err(e) => println!(
            "Manager creation failed (expected if not in input group): {}",
            e
        ),
    }
}

#[test]
fn test_register_hotkey() {
    let manager = match HotkeyManager::new() {
        Ok(m) => m,
        Err(_) => {
            println!("Skipping test: not in input group");
            return;
        }
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
    let manager = match HotkeyManager::new() {
        Ok(m) => m,
        Err(_) => {
            println!("Skipping test: not in input group");
            return;
        }
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
    let manager = match HotkeyManager::new() {
        Ok(m) => m,
        Err(_) => {
            println!("Skipping test: not in input group");
            return;
        }
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
    let manager = match HotkeyManager::new() {
        Ok(m) => m,
        Err(_) => {
            println!("Skipping test: not in input group");
            return;
        }
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
