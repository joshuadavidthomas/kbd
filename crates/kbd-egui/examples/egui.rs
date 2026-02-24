//! Egui keyboard event conversion via `kbd-egui`.
//!
//! Demonstrates converting egui key events to `kbd-core` types and feeding
//! them to a `Matcher`. This example constructs events directly (no GUI
//! window) to show the integration pattern.
//!
//! In a real eframe/egui app, you'd convert events from `egui::Context`'s
//! input state in your `update()` method.
//!
//! ```sh
//! cargo run -p kbd-egui --example egui
//! ```

use egui::Key as EguiKey;
use egui::Modifiers;
use kbd_core::Action;
use kbd_core::Hotkey;
use kbd_core::Key;
use kbd_core::KeyTransition;
use kbd_core::MatchResult;
use kbd_core::Matcher;
use kbd_core::Modifier;
use kbd_egui::EguiEventExt;
use kbd_egui::EguiKeyExt;
use kbd_egui::EguiModifiersExt;

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

    println!("=== kbd-egui conversion demo ===");
    println!();

    // Single key conversion
    println!("1. Key conversion:");
    let keys = [
        EguiKey::A,
        EguiKey::Enter,
        EguiKey::Escape,
        EguiKey::Space,
        EguiKey::F1,
        EguiKey::ArrowUp,
    ];
    for key in keys {
        let kbd_key = key.to_key();
        println!("  egui Key::{key:?} → kbd-core Key: {kbd_key:?}");
    }
    println!();

    // Modifier conversion
    println!("2. Modifier conversion:");
    let modifier_sets = [
        ("CTRL", Modifiers::CTRL),
        ("SHIFT", Modifiers::SHIFT),
        ("ALT", Modifiers::ALT),
        ("COMMAND", Modifiers::COMMAND),
        (
            "CTRL | SHIFT",
            Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
        ),
    ];
    for (label, mods) in modifier_sets {
        let kbd_mods = mods.to_modifiers();
        println!("  egui {label:20} → kbd-core {kbd_mods:?}");
    }
    println!();

    // Full event conversion and matcher integration
    println!("3. Full event → Matcher pipeline:");
    demo_event_pipeline(&mut matcher);

    println!("In a real eframe/egui app, use this pattern:");
    println!("  for event in &ctx.input(|i| i.events.clone()) {{");
    println!("      if let Some(hotkey) = event.to_hotkey() {{");
    println!("          match matcher.process(&hotkey, KeyTransition::Press) {{ ... }}");
    println!("      }}");
    println!("  }}");
}

fn demo_event_pipeline(matcher: &mut Matcher) {
    let events = [
        (
            "Ctrl+S",
            egui::Event::Key {
                key: EguiKey::S,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::CTRL,
            },
        ),
        (
            "Escape",
            egui::Event::Key {
                key: EguiKey::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
        ),
        (
            "A (no binding)",
            egui::Event::Key {
                key: EguiKey::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
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
                    MatchResult::Swallowed => println!("swallowed"),
                    MatchResult::Pending { .. } => println!("pending..."),
                    MatchResult::Ignored => println!("ignored"),
                }
            }
            None => println!("(unmappable or not a key event)"),
        }
    }
    println!();
}
