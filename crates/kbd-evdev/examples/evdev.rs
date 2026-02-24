//! Read evdev key events, convert to `kbd-core` types, and feed to a
//! `Matcher`.
//!
//! Requires read access to `/dev/input/` (typically via the `input` group
//! or running as root). Prints a usage message if permission is denied.
//!
//! ```sh
//! cargo run -p kbd-evdev --example evdev
//! ```

use std::path::Path;

use evdev::KeyCode;
use kbd_core::{Action, Hotkey, Key, KeyTransition, MatchResult, Matcher, Modifier};
use kbd_evdev::KeyCodeExt;

fn main() {
    println!("=== kbd-evdev conversion demo ===");
    println!();

    // Demonstrate evdev key code conversion
    println!("1. evdev KeyCode → kbd-core Key conversion:");
    let evdev_codes = [
        KeyCode::KEY_A,
        KeyCode::KEY_ENTER,
        KeyCode::KEY_ESC,
        KeyCode::KEY_LEFTCTRL,
        KeyCode::KEY_F1,
        KeyCode::KEY_SPACE,
        KeyCode::KEY_VOLUMEUP,
    ];
    for code in evdev_codes {
        let key: Key = code.to_key();
        println!("  evdev {code:?} → kbd-core {key}");
    }
    println!();

    // Set up a matcher
    let mut matcher = Matcher::new();
    matcher
        .register(
            Hotkey::new(Key::S).modifier(Modifier::Ctrl),
            Action::from(|| println!("  → Save!")),
        )
        .expect("register Ctrl+S");
    matcher
        .register(
            Hotkey::new(Key::Q).modifier(Modifier::Ctrl),
            Action::from(|| println!("  → Quit!")),
        )
        .expect("register Ctrl+Q");

    println!("2. Simulated event pipeline (no device access needed):");
    // Simulate what the engine does: convert evdev key codes and feed to matcher
    let simulated_events = [
        ("KEY_LEFTCTRL press", KeyCode::KEY_LEFTCTRL, KeyTransition::Press),
        ("KEY_S press", KeyCode::KEY_S, KeyTransition::Press),
        ("KEY_S release", KeyCode::KEY_S, KeyTransition::Release),
        ("KEY_LEFTCTRL release", KeyCode::KEY_LEFTCTRL, KeyTransition::Release),
    ];
    for (label, evdev_key, transition) in simulated_events {
        let key: Key = evdev_key.to_key();
        let hotkey = Hotkey::new(key);
        print!("  {label}: {hotkey} → ");
        match matcher.process(&hotkey, transition) {
            MatchResult::Matched { action, .. } => {
                if let Action::Callback(cb) = action {
                    cb();
                }
            }
            MatchResult::NoMatch => println!("no match"),
            MatchResult::Ignored => println!("ignored"),
            _ => println!("other"),
        }
    }
    println!();

    // Try to discover devices
    println!("3. Device discovery:");
    let input_dir = Path::new("/dev/input");
    if !input_dir.exists() {
        println!("  /dev/input not found (not running on Linux?)");
        return;
    }

    match std::fs::read_dir(input_dir) {
        Ok(entries) => {
            let mut found = false;
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("event") {
                        match evdev::Device::open(&path) {
                            Ok(device) => {
                                let dev_name = device.name().unwrap_or("(unknown)");
                                let has_keys = device
                                    .supported_keys()
                                    .is_some_and(|keys| keys.contains(KeyCode::KEY_A));
                                println!(
                                    "  {} — {} {}",
                                    path.display(),
                                    dev_name,
                                    if has_keys {
                                        "(keyboard)"
                                    } else {
                                        "(not a keyboard)"
                                    },
                                );
                                found = true;
                            }
                            Err(e) => {
                                if e.kind() == std::io::ErrorKind::PermissionDenied {
                                    println!("  {} — permission denied", path.display());
                                    println!();
                                    println!("  Tip: add your user to the 'input' group:");
                                    println!("    sudo usermod -aG input $USER");
                                    return;
                                }
                            }
                        }
                    }
                }
            }
            if !found {
                println!("  No event devices found");
            }
        }
        Err(e) => println!("  Cannot read /dev/input: {e}"),
    }
}
