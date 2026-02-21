//! Hotkey string parsing and display.
//!
//! keybound supports parsing hotkeys from human-readable strings, which
//! round-trip through `Display`. Useful for config files, user input, and
//! logging.
//!
//! ```sh
//! cargo run --example string_parsing
//! ```

use keybound::Hotkey;
use keybound::HotkeyManager;
use keybound::HotkeySequence;
use keybound::Key;
use keybound::Modifier;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse single hotkeys from strings
    let hotkey: Hotkey = "Ctrl+Shift+A".parse()?;
    println!("Parsed: {hotkey}");
    println!("  Key: {:?}", hotkey.key());
    println!("  Modifiers: {:?}", hotkey.modifiers());
    println!();

    // Various accepted formats
    let examples = [
        "Ctrl+A",
        "Shift+Alt+Delete",
        "Super+L",
        "Ctrl+Shift+F12",
        "Escape",
        "Ctrl+Space",
        "Alt+Tab",
        "Ctrl+Shift+Alt+Super+X",
    ];

    println!("Parsing examples:");
    for input in examples {
        match input.parse::<Hotkey>() {
            Ok(hk) => println!("  {input:30} → {hk}"),
            Err(err) => println!("  {input:30} → ERROR: {err}"),
        }
    }
    println!();

    // Parse key sequences (comma-separated hotkeys)
    let seq: HotkeySequence = "Ctrl+K, Ctrl+C".parse()?;
    println!("Parsed sequence: {seq}");
    println!("  Steps: {}", seq.steps().len());
    for (i, step) in seq.steps().iter().enumerate() {
        println!("    Step {}: {step}", i + 1);
    }
    println!();

    // Round-trip: construct programmatically, display as string
    let hotkey = Hotkey::new(Key::L, vec![Modifier::Super]);
    println!("Constructed: {hotkey}");

    // Error cases
    println!("Error handling:");
    let bad_inputs = ["", "Ctrl+", "Ctrl+Unknown", "Ctrl+A+B"];
    for input in bad_inputs {
        match input.parse::<Hotkey>() {
            Ok(hk) => println!("  \"{input}\" → {hk}"),
            Err(err) => println!("  \"{input}\" → {err}"),
        }
    }
    println!();

    // Use parsed hotkeys with the manager
    let manager = HotkeyManager::new()?;
    let hotkey: Hotkey = "Ctrl+Shift+P".parse()?;
    let display = hotkey.to_string();
    let _handle = manager.register(hotkey.key(), hotkey.modifiers(), move || {
        println!("Triggered: {display}");
    })?;

    println!("Registered Ctrl+Shift+P from parsed string");
    println!("Press Ctrl+C to exit");

    std::thread::park();
    Ok(())
}
