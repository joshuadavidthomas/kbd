//! Backend selection and detection.
//!
//! keybound automatically picks the best backend for your environment:
//! - **Portal**: XDG `GlobalShortcuts` portal (Wayland, sandboxed apps)
//! - **Evdev**: Direct /dev/input access (works everywhere on Linux)
//!
//! You can also force a specific backend or detect what would be selected.
//!
//! ```sh
//! cargo run --example backend_selection --features evdev
//! ```

use keybound::HotkeyManager;
use keybound::Key;
use keybound::Modifier;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Detect which backend would be selected without creating a manager.
    // Useful for showing users which mode they're in.
    match HotkeyManager::detect_backend(None) {
        Ok(backend) => println!("Auto-detected backend: {backend:?}"),
        Err(err) => println!("No backend available: {err}"),
    }

    // Auto-select: tries Portal first, falls back to Evdev.
    let manager = HotkeyManager::new()?;
    println!(
        "Manager created with backend: {:?}",
        manager.active_backend()
    );

    // Or force a specific backend:
    //   let manager = HotkeyManager::with_backend(Backend::Evdev)?;

    // Or use the builder for more control:
    //   let manager = HotkeyManager::builder()
    //       .backend(Backend::Evdev)
    //       .build()?;

    let _handle = manager.register(Key::B, &[Modifier::Ctrl, Modifier::Shift], || {
        println!("Hotkey triggered!");
    })?;

    println!(
        "Press Ctrl+Shift+B to trigger (backend: {:?})",
        manager.active_backend()
    );
    println!("Press Ctrl+C to exit");

    std::thread::park();
    Ok(())
}
