//! Configuration file — load hotkeys from TOML.
//!
//! keybound supports loading hotkey configurations from serialized formats
//! (TOML, JSON, YAML). The config maps hotkey combos to named action IDs,
//! and your application provides the callbacks via an `ActionMap`.
//!
//! Requires the `serde` feature.
//!
//! ```sh
//! cargo run --example config_file --features serde
//! ```

use keybound::ActionId;
use keybound::ActionMap;
use keybound::HotkeyConfig;
use keybound::HotkeyManager;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Configuration file example");
    println!();

    // Load configuration from TOML
    let config_toml = include_str!("config.toml");
    let config: HotkeyConfig = toml::from_str(config_toml)?;

    println!("Loaded config:");
    println!("  {} hotkeys", config.hotkeys().len());
    println!("  {} sequences", config.sequences().len());
    println!("  {} modes", config.modes().len());
    println!();

    // Build the action map: connect action IDs to actual callbacks
    let mut actions = ActionMap::new();

    actions.insert("new_window".parse::<ActionId>()?, || {
        println!("[action] new_window → opening new window");
    })?;

    actions.insert("quit".parse::<ActionId>()?, || {
        println!("[action] quit → shutting down");
    })?;

    actions.insert("toggle_fullscreen".parse::<ActionId>()?, || {
        println!("[action] toggle_fullscreen → toggling");
    })?;

    actions.insert("save_all".parse::<ActionId>()?, || {
        println!("[action] save_all → saving all files");
    })?;

    actions.insert("open_recent".parse::<ActionId>()?, || {
        println!("[action] open_recent → opening recent files");
    })?;

    actions.insert("shrink_left".parse::<ActionId>()?, || {
        println!("[action] shrink_left");
    })?;

    actions.insert("shrink_down".parse::<ActionId>()?, || {
        println!("[action] shrink_down");
    })?;

    actions.insert("grow_up".parse::<ActionId>()?, || {
        println!("[action] grow_up");
    })?;

    actions.insert("grow_right".parse::<ActionId>()?, || {
        println!("[action] grow_right");
    })?;

    actions.insert("exit_mode".parse::<ActionId>()?, || {
        println!("[action] exit_mode");
    })?;

    // Register everything in one call
    let manager = HotkeyManager::new()?;
    let registered = config.register(&manager, &actions)?;

    println!("Registered from config:");
    println!("  {} hotkey handles", registered.hotkey_handles().len());
    println!("  {} sequence handles", registered.sequence_handles().len());
    println!("  {} modes defined", registered.defined_modes().len());
    println!();
    println!("Hotkeys:");
    println!("  Ctrl+Shift+N  → new_window");
    println!("  Ctrl+Shift+Q  → quit");
    println!("  Ctrl+Shift+F  → toggle_fullscreen");
    println!("Sequences:");
    println!("  Ctrl+K, Ctrl+S  → save_all");
    println!("  Ctrl+K, Ctrl+O  → open_recent");
    println!("Modes:");
    println!("  resize: h/j/k/l/Escape");
    println!();

    // The config can also be loaded from JSON
    let json_config = r#"{
        "hotkeys": [
            { "hotkey": "Ctrl+J", "action": "new_window" }
        ],
        "sequences": [],
        "modes": {}
    }"#;
    let _json_parsed: HotkeyConfig = serde_json::from_str(json_config)?;
    println!("JSON config also parses successfully.");

    println!();
    println!("Press Ctrl+C to exit");

    std::thread::park();
    Ok(())
}
