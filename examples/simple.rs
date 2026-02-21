use keybound::HotkeyManager;
use keybound::Key;
use keybound::Modifier;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting keybound example");
    println!("Press Ctrl+Shift+C to trigger hotkey");

    let manager = HotkeyManager::new()?;

    let _handle = manager.register(Key::C, &[Modifier::Ctrl, Modifier::Shift], || {
        println!("Hotkey triggered! (Ctrl+Shift+C)");
    })?;

    println!("Hotkey registered. Waiting for input...");
    println!("Press Ctrl+C to exit");

    std::thread::park();

    Ok(())
}
