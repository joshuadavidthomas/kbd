//! Multiple hotkeys — same key with different modifiers.
//!
//! Registers several hotkeys at once, including the same target key (`X`)
//! with different modifier combinations to show they are independent.
//!
//! ```sh
//! cargo run --example multi
//! ```

use keybound::HotkeyManager;
use keybound::Key;
use keybound::Modifier;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Multiple hotkeys example");
    println!();

    let manager = HotkeyManager::new()?;

    let _handle1 = manager.register(Key::A, &[Modifier::Ctrl, Modifier::Shift], || {
        println!("Action A triggered!");
    })?;

    let _handle2 = manager.register(Key::B, &[Modifier::Ctrl, Modifier::Shift], || {
        println!("Action B triggered!");
    })?;

    // Same key (X) with different modifiers
    let _handle3 = manager.register(Key::X, &[Modifier::Ctrl, Modifier::Alt], || {
        println!("Action X1 triggered! (Ctrl+Alt+X)");
    })?;

    let _handle4 = manager.register(Key::X, &[Modifier::Ctrl, Modifier::Shift], || {
        println!("Action X2 triggered! (Ctrl+Shift+X)");
    })?;

    println!("  Ctrl+Shift+A  → Action A");
    println!("  Ctrl+Shift+B  → Action B");
    println!("  Ctrl+Alt+X    → Action X1");
    println!("  Ctrl+Shift+X  → Action X2");
    println!();
    println!("Press Ctrl+C to exit");

    std::thread::park();

    Ok(())
}
