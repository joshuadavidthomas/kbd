use evdev::KeyCode;
use evdev_hotkey::HotkeyManager;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting evdev-hotkey example");
    println!("Press Ctrl+Shift+C to trigger hotkey");

    let manager = HotkeyManager::new()?;

    let _handle = manager.register(
        KeyCode::KEY_C,
        &[KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTSHIFT],
        || {
            println!("Hotkey triggered! (Ctrl+Shift+C)");
        },
    )?;

    println!("Hotkey registered. Waiting for input...");
    println!("Press Ctrl+C to exit");

    std::thread::park();

    Ok(())
}
