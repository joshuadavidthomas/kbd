//! Standalone `Dispatcher` usage — register bindings, process key events,
//! print match results.
//!
//! This example shows `kbd` works on its own: no platform dependencies,
//! no threads, no async. You bring the events, the `Dispatcher` tells you what
//! matched.
//!
//! ```sh
//! cargo run -p kbd --example matcher
//! ```

use kbd::Action;
use kbd::Hotkey;
use kbd::Key;
use kbd::KeyTransition;
use kbd::MatchResult;
use kbd::Dispatcher;
use kbd::Modifier;

fn main() {
    let mut matcher = Dispatcher::new();

    // Register some bindings
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

    matcher
        .register(
            Hotkey::new(Key::Z)
                .modifier(Modifier::Ctrl)
                .modifier(Modifier::Shift),
            Action::from(|| println!("  → Redo!")),
        )
        .expect("register Ctrl+Shift+Z");

    println!("Registered bindings:");
    println!("  Ctrl+S  → Save");
    println!("  Ctrl+Q  → Quit");
    println!("  Ctrl+Shift+Z  → Redo");
    println!();

    // Simulate key events — in a real app these come from your event loop
    let test_events = [
        ("Ctrl+S", Hotkey::new(Key::S).modifier(Modifier::Ctrl)),
        ("Ctrl+Q", Hotkey::new(Key::Q).modifier(Modifier::Ctrl)),
        (
            "Ctrl+Shift+Z",
            Hotkey::new(Key::Z)
                .modifier(Modifier::Ctrl)
                .modifier(Modifier::Shift),
        ),
        ("A (no match)", Hotkey::new(Key::A)),
        (
            "Ctrl+A (no match)",
            Hotkey::new(Key::A).modifier(Modifier::Ctrl),
        ),
    ];

    println!("Processing events:");
    for (label, hotkey) in &test_events {
        print!("  {label}: ");
        match matcher.process(hotkey, KeyTransition::Press) {
            MatchResult::Matched { action, .. } => {
                if let Action::Callback(cb) = action {
                    cb();
                }
            }
            MatchResult::Pending {
                steps_matched,
                steps_remaining,
            } => {
                println!("  → Pending (matched {steps_matched}, remaining {steps_remaining})");
            }
            MatchResult::NoMatch => println!("  → No match"),
            MatchResult::Suppressed => println!("  → Suppressed"),
            MatchResult::Ignored => println!("  → Ignored"),
        }
    }
}
