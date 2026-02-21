use evdev::KeyCode;
use keybound::HotkeyManager;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting keybound example");
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
