//! Minimal hotkey registration.
//!
//! Registers a single hotkey and waits for it to fire. This is the
//! simplest possible use of keybound.
//!
//! ```sh
//! cargo run --example simple
//! ```

use keybound::HotkeyManager;
use keybound::Key;
use keybound::Modifier;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Minimal hotkey example");
    println!();

    let manager = HotkeyManager::new()?;

    let _handle = manager.register(Key::C, &[Modifier::Ctrl, Modifier::Shift], || {
        println!("Hotkey triggered!");
    })?;

    println!("  Ctrl+Shift+C  → trigger hotkey");
    println!();
    println!("Press Ctrl+C to exit");

    std::thread::park();

    Ok(())
}
