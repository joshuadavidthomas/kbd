//! Parse hotkeys from strings, display them back.
//!
//! `kbd-core` supports parsing hotkeys from human-readable strings that
//! round-trip through `Display`. Useful for config files, user input,
//! and logging.
//!
//! ```sh
//! cargo run -p kbd-core --example string_parsing
//! ```

use kbd_core::{Hotkey, HotkeySequence, Key, Modifier};

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

    // Case-insensitive parsing with aliases
    let aliases = [
        ("ctrl+a", "lowercase"),
        ("CTRL+A", "uppercase"),
        ("Control+A", "Control alias"),
        ("Meta+A", "Meta alias for Super"),
        ("Win+A", "Win alias for Super"),
        ("Return", "Return alias for Enter"),
    ];

    println!("Aliases and case-insensitivity:");
    for (input, note) in aliases {
        match input.parse::<Hotkey>() {
            Ok(hk) => println!("  {input:20} ({note:25}) → {hk}"),
            Err(err) => println!("  {input:20} ({note:25}) → ERROR: {err}"),
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

    // Round-trip: construct programmatically, display as string, parse back
    let original = Hotkey::new(Key::L).modifier(Modifier::Super);
    let text = original.to_string();
    let parsed: Hotkey = text.parse()?;
    println!("Round-trip:");
    println!("  Constructed: {original}");
    println!("  As string:   {text}");
    println!("  Parsed back: {parsed}");
    println!(
        "  Equal: {}",
        if original == parsed { "yes" } else { "no" }
    );
    println!();

    // Error cases — these all fail gracefully
    let bad_inputs = ["", "Ctrl+", "+A", "Ctrl+Unknown"];
    println!("Error handling:");
    for input in bad_inputs {
        match input.parse::<Hotkey>() {
            Ok(hk) => println!("  \"{input}\" → {hk}"),
            Err(err) => println!("  \"{input}\" → {err}"),
        }
    }

    Ok(())
}
