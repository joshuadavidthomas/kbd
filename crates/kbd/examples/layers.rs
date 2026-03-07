//! Layer stack walkthrough — define layers, push/pop/toggle, show how
//! bindings shadow and fall through.
//!
//! Layers are named groups of bindings that stack. The most recently
//! pushed layer is checked first. If no binding matches, the next layer
//! down is checked, and so on down to the global bindings.
//!
//! ```sh
//! cargo run -p kbd --example layers
//! ```

use std::time::Duration;

use kbd::action::Action;
use kbd::dispatcher::Dispatcher;
use kbd::dispatcher::MatchResult;
use kbd::hotkey::Hotkey;
use kbd::hotkey::Modifier;
use kbd::key::Key;
use kbd::key_state::KeyTransition;
use kbd::layer::Layer;

fn main() {
    let mut dispatcher = setup_dispatcher();

    println!("=== Layer stack demo ===");
    println!();

    // Without any layers, global bindings fire
    println!("1. Global bindings (no layers active):");
    process(&mut dispatcher, "H", Hotkey::new(Key::H));
    process(
        &mut dispatcher,
        "Ctrl+Q",
        Hotkey::new(Key::Q).modifier(Modifier::Ctrl),
    );
    println!();

    // Push nav layer — H now fires nav binding instead of global
    println!("2. Push 'nav' layer — H is shadowed:");
    dispatcher.push_layer("nav").expect("push nav");
    process(&mut dispatcher, "H", Hotkey::new(Key::H));
    process(&mut dispatcher, "J", Hotkey::new(Key::J));
    // Ctrl+Q falls through to global
    process(
        &mut dispatcher,
        "Ctrl+Q",
        Hotkey::new(Key::Q).modifier(Modifier::Ctrl),
    );
    // X has no binding in nav → falls through to global → no match
    process(&mut dispatcher, "X", Hotkey::new(Key::X));
    dispatcher.pop_layer().expect("pop nav");
    println!();

    // Oneshot layer — auto-pops after one keypress
    println!("3. Oneshot 'launcher' layer — pops after one key:");
    dispatcher.push_layer("launcher").expect("push launcher");
    println!("  Active layers: {:?}", layer_names(&dispatcher));
    process(&mut dispatcher, "F", Hotkey::new(Key::F));
    println!(
        "  Active layers after keypress: {:?}",
        layer_names(&dispatcher),
    );
    println!();

    // Swallow layer — unmatched keys are consumed
    println!("4. Swallow 'confirm' layer — unmatched keys consumed:");
    dispatcher.push_layer("confirm").expect("push confirm");
    process(&mut dispatcher, "Y", Hotkey::new(Key::Y));
    process(&mut dispatcher, "X", Hotkey::new(Key::X));
    // Escape pops via Action::PopLayer
    process(&mut dispatcher, "Escape", Hotkey::new(Key::ESCAPE));
    println!(
        "  Active layers after Escape: {:?}",
        layer_names(&dispatcher),
    );
    println!();

    // Toggle layer
    println!("5. Toggle 'nav' layer:");
    println!("  Before toggle: {:?}", layer_names(&dispatcher));
    dispatcher.toggle_layer("nav").expect("toggle nav on");
    println!("  After toggle on: {:?}", layer_names(&dispatcher));
    dispatcher.toggle_layer("nav").expect("toggle nav off");
    println!("  After toggle off: {:?}", layer_names(&dispatcher));
}

fn process(dispatcher: &mut Dispatcher, label: &str, hotkey: Hotkey) {
    print!("  {label}: ");
    match dispatcher.process(hotkey, KeyTransition::Press) {
        MatchResult::Matched { action, .. } => {
            if let Action::Callback(cb) = action {
                cb();
            } else {
                println!("  → Action: {action:?}");
            }
        }
        MatchResult::NoMatch => println!("  → No match"),
        MatchResult::Suppressed => println!("  → Suppressed (consumed by layer)"),
        MatchResult::Ignored => println!("  → Ignored"),
        _ => {}
    }
}

fn layer_names(dispatcher: &Dispatcher) -> Vec<String> {
    dispatcher
        .active_layers()
        .into_iter()
        .map(|info| info.name.to_string())
        .collect()
}

fn setup_dispatcher() -> Dispatcher {
    let mut dispatcher = Dispatcher::new();

    // Global bindings — always active, like a base layer
    dispatcher
        .register(
            Hotkey::new(Key::Q).modifier(Modifier::Ctrl),
            Action::from(|| println!("  → [global] Quit")),
        )
        .expect("register Ctrl+Q");
    dispatcher
        .register(
            Hotkey::new(Key::H),
            Action::from(|| println!("  → [global] Help")),
        )
        .expect("register H");

    // Define a "nav" layer with hjkl arrow-key bindings
    let nav = Layer::new("nav")
        .bind(
            Hotkey::new(Key::H),
            Action::from(|| println!("  → [nav] ← Left")),
        )
        .unwrap()
        .bind(
            Hotkey::new(Key::J),
            Action::from(|| println!("  → [nav] ↓ Down")),
        )
        .unwrap()
        .bind(
            Hotkey::new(Key::K),
            Action::from(|| println!("  → [nav] ↑ Up")),
        )
        .unwrap()
        .bind(
            Hotkey::new(Key::L),
            Action::from(|| println!("  → [nav] → Right")),
        )
        .unwrap()
        .description("Navigation layer — hjkl as arrow keys");
    dispatcher.define_layer(nav).expect("define nav layer");

    // Define a oneshot "launcher" layer — auto-pops after one keypress
    let launcher = Layer::new("launcher")
        .bind(
            Hotkey::new(Key::F),
            Action::from(|| println!("  → [launcher] Open file manager")),
        )
        .unwrap()
        .bind(
            Hotkey::new(Key::B),
            Action::from(|| println!("  → [launcher] Open browser")),
        )
        .unwrap()
        .bind(
            Hotkey::new(Key::T),
            Action::from(|| println!("  → [launcher] Open terminal")),
        )
        .unwrap()
        .oneshot(1)
        .description("App launcher — auto-pops after one key");
    dispatcher.define_layer(launcher).expect("define launcher");

    // Define a "confirm" layer that swallows unmatched keys
    let confirm = Layer::new("confirm")
        .bind(
            Hotkey::new(Key::Y),
            Action::from(|| println!("  → [confirm] YES")),
        )
        .unwrap()
        .bind(
            Hotkey::new(Key::N),
            Action::from(|| println!("  → [confirm] NO")),
        )
        .unwrap()
        .bind(Hotkey::new(Key::ESCAPE), Action::PopLayer)
        .unwrap()
        .swallow()
        .description("Confirmation dialog — only y/n/Escape work");
    dispatcher.define_layer(confirm).expect("define confirm");

    // Define a timeout layer — auto-pops after inactivity
    let quick = Layer::new("quick")
        .bind(
            Hotkey::new(Key::DIGIT1),
            Action::from(|| println!("  → [quick] Workspace 1")),
        )
        .unwrap()
        .bind(
            Hotkey::new(Key::DIGIT2),
            Action::from(|| println!("  → [quick] Workspace 2")),
        )
        .unwrap()
        .timeout(Duration::from_secs(3))
        .description("Quick workspace switcher — 3s timeout");
    dispatcher.define_layer(quick).expect("define quick");

    dispatcher
}
