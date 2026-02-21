//! Event grabbing — intercept keys before the compositor sees them.
//!
//! With grabbing enabled, matched hotkeys are consumed: the compositor and
//! applications never receive them. Unmatched keys are re-emitted via uinput
//! so normal typing continues to work.
//!
//! Use `passthrough()` on specific hotkeys to observe without consuming —
//! the callback fires AND the key reaches applications.
//!
//! Requires the `grab` feature.
//!
//! ```sh
//! cargo run --example grab --features grab
//! ```

use keybound::HotkeyManager;
use keybound::HotkeyOptions;
use keybound::Key;
use keybound::Modifier;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Event grabbing example");
    println!();

    // Enable grabbing via the builder
    let manager = HotkeyManager::builder().grab().build()?;

    // This hotkey is consumed: the compositor never sees Super+L
    let _consumed = manager.register(Key::L, &[Modifier::Super], || {
        println!("[consumed] Super+L intercepted — compositor did NOT receive it");
    })?;
    println!("  Super+L  → consumed (compositor won't lock screen)");

    // This hotkey fires the callback but also passes through to apps
    let _passthrough = manager.register_with_options(
        Key::A,
        &[Modifier::Ctrl],
        HotkeyOptions::new().passthrough(),
        || println!("[passthrough] Ctrl+A observed — apps also receive it"),
    )?;
    println!("  Ctrl+A   → passthrough (callback fires AND apps get the key)");

    // Normal hotkey with grab: consumed by default
    let _grab = manager.register(Key::G, &[Modifier::Ctrl, Modifier::Shift], || {
        println!("[consumed] Ctrl+Shift+G intercepted");
    })?;
    println!("  Ctrl+Shift+G → consumed");

    println!();
    println!("All unmatched keys pass through to applications normally.");
    println!("Press Ctrl+C to exit");

    std::thread::park();
    Ok(())
}
