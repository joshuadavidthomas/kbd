//! Full `HotkeyManager` lifecycle — register hotkeys, define layers,
//! query introspection, and shut down.
//!
//! This example requires access to `/dev/input/` devices. It will print
//! a helpful error if permissions are missing.
//!
//! ```sh
//! cargo run -p kbd-global --example global
//! ```

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use kbd_global::Action;
use kbd_global::BindingOptions;
use kbd_global::Error;
use kbd_global::Hotkey;
use kbd_global::HotkeyManager;
use kbd_global::Key;
use kbd_global::Layer;
use kbd_global::Modifier;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match run() {
        Ok(()) => Ok(()),
        Err(e) => {
            // Provide helpful guidance for common errors
            let msg = format!("{e}");
            if msg.contains("Permission denied") || msg.contains("permission") {
                eprintln!("Error: {e}");
                eprintln!();
                eprintln!("This example needs access to /dev/input/ devices.");
                eprintln!("Either:");
                eprintln!("  1. Add your user to the 'input' group:");
                eprintln!("     sudo usermod -aG input $USER");
                eprintln!("  2. Or run with sudo (not recommended for regular use)");
                Ok(())
            } else {
                Err(e.into())
            }
        }
    }
}

fn run() -> Result<(), Error> {
    println!("=== kbd-global HotkeyManager example ===");
    println!();

    let manager = HotkeyManager::new()?;
    println!("Backend: {:?}", manager.active_backend());
    println!();

    // Simple registration — closure auto-converts to Action::Callback
    let _save = manager.register(Hotkey::new(Key::S).modifier(Modifier::Ctrl), || {
        println!("  → Save!");
    })?;

    // Registration with options — add metadata for introspection
    let _quit = manager.register_with_options(
        Hotkey::new(Key::Q).modifier(Modifier::Ctrl),
        Action::from(|| println!("  → Quit!")),
        BindingOptions::default().with_description("Quit application"),
    )?;

    let _help = manager.register(Hotkey::new(Key::F1), || println!("  → Help!"))?;

    println!("Registered hotkeys:");
    println!("  Ctrl+S  → Save");
    println!("  Ctrl+Q  → Quit");
    println!("  F1      → Help");
    println!();

    // Query registration state
    let ctrl_s = Hotkey::new(Key::S).modifier(Modifier::Ctrl);
    println!(
        "  is_registered(Ctrl+S): {}",
        manager.is_registered(ctrl_s)?,
    );
    let ctrl_a = Hotkey::new(Key::A).modifier(Modifier::Ctrl);
    println!(
        "  is_registered(Ctrl+A): {}",
        manager.is_registered(ctrl_a)?,
    );
    println!();

    // Conflict detection — duplicate registration returns an error
    println!("Conflict detection:");
    match manager.register(Hotkey::new(Key::S).modifier(Modifier::Ctrl), || {
        println!("duplicate");
    }) {
        Ok(_) => println!("  (unexpected success)"),
        Err(e) => println!("  Duplicate rejected: {e}"),
    }
    println!();

    // Define a layer
    let nav = Layer::new("nav")
        .bind(
            Hotkey::new(Key::H),
            Action::from(|| println!("  → [nav] ← Left")),
        )
        .bind(
            Hotkey::new(Key::J),
            Action::from(|| println!("  → [nav] ↓ Down")),
        )
        .bind(
            Hotkey::new(Key::K),
            Action::from(|| println!("  → [nav] ↑ Up")),
        )
        .bind(
            Hotkey::new(Key::L),
            Action::from(|| println!("  → [nav] → Right")),
        )
        .bind(Hotkey::new(Key::ESCAPE), Action::PopLayer)
        .description("Navigation layer — hjkl as arrows, Escape to exit");
    manager.define_layer(nav)?;

    // Push/pop layers
    println!("Layer operations:");
    println!("  Active layers: {:?}", layer_names(&manager)?);
    manager.push_layer("nav")?;
    println!("  After push 'nav': {:?}", layer_names(&manager)?);
    let popped = manager.pop_layer()?;
    println!(
        "  After pop: {:?} (popped: {popped})",
        layer_names(&manager)?,
    );
    println!();

    // Introspection
    println!("Introspection:");
    let bindings = manager.list_bindings()?;
    println!("  Total bindings: {}", bindings.len());
    for b in &bindings {
        let desc = b.description.as_deref().unwrap_or("(no description)");
        println!("    {}: {} [{:?}]", b.hotkey, desc, b.shadowed);
    }
    println!();

    // Use shared state in callbacks via Arc
    let counter = Arc::new(AtomicBool::new(false));
    let counter_clone = Arc::clone(&counter);
    let _counted = manager.register(Hotkey::new(Key::F2), move || {
        counter_clone.store(true, Ordering::Relaxed);
        println!("  → F2 pressed (shared state updated)");
    })?;

    println!("Press registered hotkeys to test. Ctrl+C to exit.");
    println!("  Ctrl+S  → Save");
    println!("  Ctrl+Q  → Quit");
    println!("  F1      → Help");
    println!("  F2      → Shared state demo");

    // In a real app you'd park the thread or run your event loop.
    // For this example, we just wait briefly to show it's running.
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Explicit shutdown (also happens on Drop)
    manager.shutdown()?;
    println!();
    println!("Manager shut down cleanly.");

    Ok(())
}

fn layer_names(manager: &HotkeyManager) -> Result<Vec<String>, Error> {
    Ok(manager
        .active_layers()?
        .into_iter()
        .map(|info| info.name.to_string())
        .collect())
}
