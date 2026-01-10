use evdev::KeyCode;
use evdev_hotkey::HotkeyManager;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Multiple hotkeys example");
    println!("Registered hotkeys:");
    println!("  - Ctrl+Shift+A: Action A");
    println!("  - Ctrl+Shift+B: Action B");
    println!("  - Ctrl+Alt+X: Action X1 (same key, different modifiers)");
    println!("  - Ctrl+Shift+X: Action X2 (same key, different modifiers)");
    println!("Press Ctrl+C to exit");

    let manager = HotkeyManager::new()?;

    let _handle1 = manager.register(
        KeyCode::KEY_A,
        &[KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTSHIFT],
        || {
            println!("Action A triggered!");
        },
    )?;

    let _handle2 = manager.register(
        KeyCode::KEY_B,
        &[KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTSHIFT],
        || {
            println!("Action B triggered!");
        },
    )?;

    // Same key (X) with different modifiers - demonstrates registration key is (key, modifiers)
    let _handle3 = manager.register(
        KeyCode::KEY_X,
        &[KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTALT],
        || {
            println!("Action X1 triggered! (Ctrl+Alt+X)");
        },
    )?;

    let _handle4 = manager.register(
        KeyCode::KEY_X,
        &[KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTSHIFT],
        || {
            println!("Action X2 triggered! (Ctrl+Shift+X)");
        },
    )?;

    println!("All hotkeys registered. Waiting for input...");
    println!("Press Ctrl+C to exit");

    std::thread::park();

    Ok(())
}
