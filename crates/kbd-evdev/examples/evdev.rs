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
use kbd_core::Action;
use kbd_core::Hotkey;
use kbd_core::Key;
use kbd_core::KeyTransition;
use kbd_core::MatchResult;
use kbd_core::Matcher;
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

    // Set up a matcher with simple (non-modifier) bindings.
    // evdev gives raw key events without modifier state tracking —
    // for Ctrl+S style hotkeys, use kbd-global which handles that.
    let mut matcher = Matcher::new();
    matcher
        .register(
            Hotkey::new(Key::A),
            Action::from(|| println!("  → A pressed!")),
        )
        .expect("register A");
    matcher
        .register(
            Hotkey::new(Key::ESCAPE),
            Action::from(|| println!("  → Escape!")),
        )
        .expect("register Escape");

    println!("2. Simulated event pipeline (no device access needed):");
    let simulated_events = [
        ("KEY_A press", KeyCode::KEY_A, KeyTransition::Press),
        ("KEY_A release", KeyCode::KEY_A, KeyTransition::Release),
        ("KEY_ESC press", KeyCode::KEY_ESC, KeyTransition::Press),
        (
            "KEY_B press (no binding)",
            KeyCode::KEY_B,
            KeyTransition::Press,
        ),
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
            MatchResult::Swallowed => println!("swallowed"),
            MatchResult::Pending { .. } => println!("pending..."),
            MatchResult::Ignored => println!("ignored"),
        }
    }
    println!();

    // Try to discover devices
    println!("3. Device discovery:");
    discover_devices();
}

fn discover_devices() {
    let input_dir = Path::new("/dev/input");
    if !input_dir.exists() {
        println!("  /dev/input not found (not running on Linux?)");
        return;
    }

    let entries = match std::fs::read_dir(input_dir) {
        Ok(entries) => entries,
        Err(e) => {
            println!("  Cannot read /dev/input: {e}");
            return;
        }
    };

    let mut found = false;
    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(name) = path.file_name().and_then(|n| n.to_str())
            && name.starts_with("event")
        {
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
    if !found {
        println!("  No event devices found");
    }
}
