//! Key state queries — check what's currently pressed.
//!
//! The manager tracks real-time key state across all devices.
//! Query whether specific keys are pressed or which modifiers are active,
//! useful for complex conditional logic inside callbacks.
//!
//! ```sh
//! cargo run --example key_state
//! ```

use std::sync::Arc;
use std::time::Duration;

use keybound::HotkeyManager;
use keybound::Key;
use keybound::Modifier;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Key state query example");
    println!();

    let manager = Arc::new(HotkeyManager::new()?);

    // Use key state queries inside a callback for conditional behavior
    let mgr = manager.clone();
    let _handle = manager.register(Key::I, &[Modifier::Ctrl], move || {
        println!("[Ctrl+I] Key state snapshot:");

        // Check specific key state
        let shift_held = mgr.is_key_pressed(Key::LeftBracket)
            || mgr.active_modifiers().contains(&Modifier::Shift);
        println!("  Shift held: {shift_held}");

        // Get all active modifiers
        let mods = mgr.active_modifiers();
        println!("  Active modifiers: {mods:?}");

        // Check for other keys
        for key in [Key::A, Key::Space, Key::CapsLock] {
            if mgr.is_key_pressed(key) {
                println!("  {key:?} is also pressed!");
            }
        }
    })?;
    println!("  Ctrl+I  → print key state snapshot");

    // Periodic state polling from a background thread
    let mgr = manager.clone();
    let _poll_handle = manager.register(Key::P, &[Modifier::Ctrl, Modifier::Shift], move || {
        let mgr = mgr.clone();
        std::thread::spawn(move || {
            println!("[Ctrl+Shift+P] Polling key state for 5 seconds...");
            for i in 0..10 {
                std::thread::sleep(Duration::from_millis(500));
                let mods = mgr.active_modifiers();
                if !mods.is_empty() {
                    println!("  [{:.1}s] modifiers: {mods:?}", f64::from(i + 1) * 0.5);
                }
            }
            println!("  Polling complete.");
        });
    })?;
    println!("  Ctrl+Shift+P  → poll modifiers for 5 seconds");

    println!();
    println!("Press Ctrl+C to exit");

    std::thread::park();
    Ok(())
}
