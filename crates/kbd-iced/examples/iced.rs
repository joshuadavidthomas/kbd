//! Iced keyboard event conversion via `kbd-iced`.
//!
//! Demonstrates converting iced keyboard events to `kbd-core` types and
//! feeding them to a `Matcher`. This example constructs events directly
//! (no GUI window) to show the integration pattern.
//!
//! In a real iced app, you'd receive keyboard events in your `update()`
//! method and convert them there.
//!
//! ```sh
//! cargo run -p kbd-iced --example iced
//! ```

use iced_core::keyboard::key::{Code, Physical};
use iced_core::keyboard::{Event, Modifiers};
use kbd_core::{Action, Hotkey, Key, KeyTransition, MatchResult, Matcher, Modifier};
use kbd_iced::{IcedEventExt, IcedKeyExt, IcedModifiersExt};

fn main() {
    let mut matcher = Matcher::new();

    matcher
        .register(
            Hotkey::new(Key::S).modifier(Modifier::Ctrl),
            Action::from(|| println!("  → Save!")),
        )
        .expect("register Ctrl+S");
    matcher
        .register(
            Hotkey::new(Key::ESCAPE),
            Action::from(|| println!("  → Escape!")),
        )
        .expect("register Escape");

    println!("=== kbd-iced conversion demo ===");
    println!();

    // Single key conversion
    println!("1. Key code conversion:");
    let codes = [Code::KeyA, Code::Enter, Code::Escape, Code::F1];
    for code in codes {
        let key = code.to_key();
        println!("  iced Code::{code:?} → kbd-core Key: {key:?}");
    }
    println!();

    // Physical key conversion (wraps Code with unidentified fallback)
    println!("2. Physical key conversion:");
    let physical = Physical::Code(Code::Space);
    println!("  iced Physical::{physical:?} → kbd-core Key: {:?}", physical.to_key());
    println!();

    // Modifier conversion
    println!("3. Modifier conversion:");
    let modifier_sets = [
        ("CTRL", Modifiers::CTRL),
        ("SHIFT", Modifiers::SHIFT),
        ("ALT", Modifiers::ALT),
        ("LOGO", Modifiers::LOGO),
        ("CTRL | SHIFT", Modifiers::CTRL | Modifiers::SHIFT),
    ];
    for (label, mods) in modifier_sets {
        let kbd_mods = mods.to_modifiers();
        println!("  iced {label:20} → kbd-core {kbd_mods:?}");
    }
    println!();

    // Full event conversion and matcher integration
    println!("4. Full event → Matcher pipeline:");
    let events = [
        (
            "Ctrl+S (press)",
            Event::KeyPressed {
                key: iced_core::keyboard::Key::Unidentified,
                modified_key: iced_core::keyboard::Key::Unidentified,
                physical_key: Physical::Code(Code::KeyS),
                location: iced_core::keyboard::Location::Standard,
                modifiers: Modifiers::CTRL,
                text: None,
                repeat: false,
            },
        ),
        (
            "Escape (press)",
            Event::KeyPressed {
                key: iced_core::keyboard::Key::Unidentified,
                modified_key: iced_core::keyboard::Key::Unidentified,
                physical_key: Physical::Code(Code::Escape),
                location: iced_core::keyboard::Location::Standard,
                modifiers: Modifiers::empty(),
                text: None,
                repeat: false,
            },
        ),
    ];

    for (label, event) in &events {
        print!("  {label}: ");
        match event.to_hotkey() {
            Some(hotkey) => {
                print!("{hotkey} → ");
                match matcher.process(&hotkey, KeyTransition::Press) {
                    MatchResult::Matched { action, .. } => {
                        if let Action::Callback(cb) = action {
                            cb();
                        }
                    }
                    MatchResult::NoMatch => println!("no match"),
                    _ => println!("other"),
                }
            }
            None => println!("(unmappable)"),
        }
    }
    println!();

    println!("In a real iced app, use this pattern in your update() method:");
    println!("  if let iced::Event::Keyboard(event) = &event {{");
    println!("      if let Some(hotkey) = event.to_hotkey() {{");
    println!("          match matcher.process(&hotkey, KeyTransition::Press) {{ ... }}");
    println!("      }}");
    println!("  }}");
}
