//! Conflict detection and replacement.
//!
//! keybound prevents duplicate registrations by default and returns a clear
//! error. Use `replace()` to intentionally swap a callback, or use RAII
//! handles to manage lifetimes.
//!
//! ```sh
//! cargo run --example conflict_detection
//! ```

use keybound::HotkeyManager;
use keybound::Key;
use keybound::Modifier;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Conflict detection and replacement example");
    println!();

    let manager = HotkeyManager::new()?;

    // Register a hotkey
    let _handle = manager.register(Key::A, &[Modifier::Ctrl], || {
        println!("First callback for Ctrl+A");
    })?;
    println!("  Registered Ctrl+A (first callback)");

    // Trying to register the same combo again is an error
    match manager.register(Key::A, &[Modifier::Ctrl], || {
        println!("This won't be reached");
    }) {
        Ok(_) => println!("  Unexpected success!"),
        Err(err) => println!("  Duplicate rejected: {err}"),
    }
    println!();

    // Check if a hotkey is already registered
    println!(
        "  is_registered(Ctrl+A): {}",
        manager.is_registered(Key::A, &[Modifier::Ctrl])
    );
    println!(
        "  is_registered(Ctrl+B): {}",
        manager.is_registered(Key::B, &[Modifier::Ctrl])
    );
    println!();

    // Use replace() to swap the callback without unregistering first
    let _replaced = manager.replace(Key::A, &[Modifier::Ctrl], || {
        println!("Replaced callback for Ctrl+A!");
    })?;
    println!("  Replaced Ctrl+A callback");

    // RAII: dropping a handle unregisters the hotkey
    {
        let handle = manager.register(Key::B, &[Modifier::Ctrl], || {
            println!("Ctrl+B in scope");
        })?;
        println!("  Registered Ctrl+B (in scope)");
        println!(
            "  is_registered(Ctrl+B): {}",
            manager.is_registered(Key::B, &[Modifier::Ctrl])
        );

        // Explicit unregister is also available
        handle.unregister()?;
        println!("  Unregistered Ctrl+B");
        println!(
            "  is_registered(Ctrl+B): {}",
            manager.is_registered(Key::B, &[Modifier::Ctrl])
        );
    }
    println!();

    // replace() also works for new registrations (register-or-replace)
    let _new = manager.replace(Key::D, &[Modifier::Ctrl], || {
        println!("Ctrl+D (via replace on new key)");
    })?;
    println!("  replace() on unregistered key creates it");

    println!();
    println!("Press Ctrl+A to test the replaced callback");
    println!("Press Ctrl+D to test the new registration");
    println!("Press Ctrl+C to exit");

    std::thread::park();
    Ok(())
}
