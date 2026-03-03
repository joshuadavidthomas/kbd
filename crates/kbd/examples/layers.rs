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

use kbd::Action;
use kbd::Dispatcher;
use kbd::Hotkey;
use kbd::Key;
use kbd::KeyTransition;
use kbd::Layer;
use kbd::MatchResult;
use kbd::Modifier;

fn main() {
    let mut matcher = setup_matcher();

    println!("=== Layer stack demo ===");
    println!();

    // Without any layers, global bindings fire
    println!("1. Global bindings (no layers active):");
    process(&mut matcher, "H", &Hotkey::new(Key::H));
    process(
        &mut matcher,
        "Ctrl+Q",
        &Hotkey::new(Key::Q).modifier(Modifier::Ctrl),
    );
    println!();

    // Push nav layer — H now fires nav binding instead of global
    println!("2. Push 'nav' layer — H is shadowed:");
    matcher.push_layer("nav").expect("push nav");
    process(&mut matcher, "H", &Hotkey::new(Key::H));
    process(&mut matcher, "J", &Hotkey::new(Key::J));
    // Ctrl+Q falls through to global
    process(
        &mut matcher,
        "Ctrl+Q",
        &Hotkey::new(Key::Q).modifier(Modifier::Ctrl),
    );
    // X has no binding in nav → falls through to global → no match
    process(&mut matcher, "X", &Hotkey::new(Key::X));
    matcher.pop_layer().expect("pop nav");
    println!();

    // Oneshot layer — auto-pops after one keypress
    println!("3. Oneshot 'launcher' layer — pops after one key:");
    matcher.push_layer("launcher").expect("push launcher");
    println!("  Active layers: {:?}", layer_names(&matcher));
    process(&mut matcher, "F", &Hotkey::new(Key::F));
    println!(
        "  Active layers after keypress: {:?}",
        layer_names(&matcher),
    );
    println!();

    // Swallow layer — unmatched keys are consumed
    println!("4. Swallow 'confirm' layer — unmatched keys consumed:");
    matcher.push_layer("confirm").expect("push confirm");
    process(&mut matcher, "Y", &Hotkey::new(Key::Y));
    process(&mut matcher, "X", &Hotkey::new(Key::X));
    // Escape pops via Action::PopLayer
    process(&mut matcher, "Escape", &Hotkey::new(Key::ESCAPE));
    println!("  Active layers after Escape: {:?}", layer_names(&matcher),);
    println!();

    // Toggle layer
    println!("5. Toggle 'nav' layer:");
    println!("  Before toggle: {:?}", layer_names(&matcher));
    matcher.toggle_layer("nav").expect("toggle nav on");
    println!("  After toggle on: {:?}", layer_names(&matcher));
    matcher.toggle_layer("nav").expect("toggle nav off");
    println!("  After toggle off: {:?}", layer_names(&matcher));
}

fn process(matcher: &mut Dispatcher, label: &str, hotkey: &Hotkey) {
    print!("  {label}: ");
    match matcher.process(hotkey, KeyTransition::Press) {
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
        MatchResult::Pending { .. } => println!("  → Pending"),
    }
}

fn layer_names(matcher: &Dispatcher) -> Vec<String> {
    matcher
        .active_layers()
        .into_iter()
        .map(|info| info.name.to_string())
        .collect()
}

fn setup_matcher() -> Dispatcher {
    let mut matcher = Dispatcher::new();

    // Global bindings — always active, like a base layer
    matcher
        .register(
            Hotkey::new(Key::Q).modifier(Modifier::Ctrl),
            Action::from(|| println!("  → [global] Quit")),
        )
        .expect("register Ctrl+Q");
    matcher
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
        .description("Navigation layer — hjkl as arrow keys");
    matcher.define_layer(nav).expect("define nav layer");

    // Define a oneshot "launcher" layer — auto-pops after one keypress
    let launcher = Layer::new("launcher")
        .bind(
            Hotkey::new(Key::F),
            Action::from(|| println!("  → [launcher] Open file manager")),
        )
        .bind(
            Hotkey::new(Key::B),
            Action::from(|| println!("  → [launcher] Open browser")),
        )
        .bind(
            Hotkey::new(Key::T),
            Action::from(|| println!("  → [launcher] Open terminal")),
        )
        .oneshot(1)
        .description("App launcher — auto-pops after one key");
    matcher.define_layer(launcher).expect("define launcher");

    // Define a "confirm" layer that swallows unmatched keys
    let confirm = Layer::new("confirm")
        .bind(
            Hotkey::new(Key::Y),
            Action::from(|| println!("  → [confirm] YES")),
        )
        .bind(
            Hotkey::new(Key::N),
            Action::from(|| println!("  → [confirm] NO")),
        )
        .bind(Hotkey::new(Key::ESCAPE), Action::PopLayer)
        .swallow()
        .description("Confirmation dialog — only y/n/Escape work");
    matcher.define_layer(confirm).expect("define confirm");

    // Define a timeout layer — auto-pops after inactivity
    let quick = Layer::new("quick")
        .bind(
            Hotkey::new(Key::DIGIT1),
            Action::from(|| println!("  → [quick] Workspace 1")),
        )
        .bind(
            Hotkey::new(Key::DIGIT2),
            Action::from(|| println!("  → [quick] Workspace 2")),
        )
        .timeout(Duration::from_secs(3))
        .description("Quick workspace switcher — 3s timeout");
    matcher.define_layer(quick).expect("define quick");

    matcher
}
