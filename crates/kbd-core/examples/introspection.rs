//! Query matcher state — `list_bindings()`, `bindings_for_key()`,
//! `active_layers()`, `conflicts()`.
//!
//! Introspection lets you build help screens, hotkey overlays, and
//! keybinding editors. Every binding carries metadata (description,
//! overlay visibility) and the matcher can tell you what's active,
//! what's shadowed, and what would fire for any given key.
//!
//! ```sh
//! cargo run -p kbd-core --example introspection
//! ```

use kbd_core::Action;
use kbd_core::BindingId;
use kbd_core::BindingInfo;
use kbd_core::BindingOptions;
use kbd_core::Hotkey;
use kbd_core::Key;
use kbd_core::Layer;
use kbd_core::Matcher;
use kbd_core::Modifier;
use kbd_core::OverlayVisibility;
use kbd_core::RegisteredBinding;
use kbd_core::ShadowedStatus;

fn main() {
    let (mut matcher, copy_id) = setup_matcher();

    println!("=== Introspection demo ===");
    println!();

    // List all bindings
    println!("1. All registered bindings:");
    print_bindings(&matcher.list_bindings());
    println!();

    // Query what would fire for a specific key
    println!("2. What fires for Ctrl+C (no layers active)?");
    let hotkey = Hotkey::new(Key::C).modifier(Modifier::Ctrl);
    match matcher.bindings_for_key(&hotkey) {
        Some(info) => println!("  {}", format_binding(&info)),
        None => println!("  (nothing)"),
    }
    println!();

    // Push the vim layer — now Ctrl+C is shadowed
    matcher.push_layer("vim-normal").expect("push vim-normal");

    println!("3. Active layers:");
    for layer in matcher.active_layers() {
        println!(
            "  {} — {} binding(s){}",
            layer.name,
            layer.binding_count,
            layer
                .description
                .as_deref()
                .map_or(String::new(), |d| format!(" ({d})")),
        );
    }
    println!();

    // List bindings again — now some are shadowed
    println!("4. All bindings with vim-normal layer active:");
    print_bindings(&matcher.list_bindings());
    println!();

    // What fires for Ctrl+C now?
    println!("5. What fires for Ctrl+C (with vim-normal layer)?");
    match matcher.bindings_for_key(&hotkey) {
        Some(info) => println!("  {}", format_binding(&info)),
        None => println!("  (nothing)"),
    }
    println!();

    // Show conflicts
    println!("6. Conflicts (shadowed bindings):");
    let conflicts = matcher.conflicts();
    if conflicts.is_empty() {
        println!("  (none)");
    } else {
        for conflict in &conflicts {
            println!(
                "  {} — {} shadows {}",
                conflict.hotkey,
                format_location(&conflict.shadowing_binding),
                format_location(&conflict.shadowed_binding),
            );
        }
    }
    println!();

    // Filter for overlay-visible bindings only
    println!("7. Overlay-visible bindings only:");
    let visible: Vec<_> = matcher
        .list_bindings()
        .into_iter()
        .filter(|b| b.overlay_visibility == OverlayVisibility::Visible)
        .collect();
    print_bindings(&visible);

    // Clean up — demonstrate that unregister works
    matcher.unregister(copy_id);
    println!();
    println!(
        "After unregistering global Ctrl+C: {} total bindings",
        matcher.list_bindings().len()
    );
}

fn print_bindings(bindings: &[BindingInfo]) {
    for binding in bindings {
        println!("  {}", format_binding(binding));
    }
}

fn format_binding(b: &BindingInfo) -> String {
    let desc = b.description.as_deref().map_or("(no description)", |d| d);
    let shadow = match &b.shadowed {
        ShadowedStatus::Active => "active".to_string(),
        ShadowedStatus::ShadowedBy(name) => format!("shadowed by {name}"),
        ShadowedStatus::Inactive => "inactive".to_string(),
    };
    let vis = match b.overlay_visibility {
        OverlayVisibility::Visible => "",
        OverlayVisibility::Hidden => " [hidden]",
    };
    format!(
        "{:20} {:30} [{}, {}]{vis}",
        b.hotkey.to_string(),
        desc,
        format_location(b),
        shadow,
    )
}

fn format_location(b: &BindingInfo) -> String {
    match &b.location {
        kbd_core::BindingLocation::Global => "global".to_string(),
        kbd_core::BindingLocation::Layer(name) => format!("layer:{name}"),
    }
}

fn setup_matcher() -> (Matcher, BindingId) {
    let mut matcher = Matcher::new();

    // Register global bindings with metadata
    let copy_id = matcher
        .register(
            Hotkey::new(Key::C).modifier(Modifier::Ctrl),
            Action::from(|| {}),
        )
        .expect("register Ctrl+C");

    matcher
        .register_binding(
            RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::V).modifier(Modifier::Ctrl),
                Action::from(|| {}),
            )
            .with_options(BindingOptions::default().with_description("Paste from clipboard")),
        )
        .expect("register Ctrl+V");

    matcher
        .register_binding(
            RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::S).modifier(Modifier::Ctrl),
                Action::from(|| {}),
            )
            .with_options(BindingOptions::default().with_description("Save file")),
        )
        .expect("register Ctrl+S");

    // A hidden binding — won't appear in overlay views
    matcher
        .register_binding(
            RegisteredBinding::new(BindingId::new(), Hotkey::new(Key::F12), Action::from(|| {}))
                .with_options(
                    BindingOptions::default()
                        .with_description("Debug panel (internal)")
                        .with_overlay_visibility(OverlayVisibility::Hidden),
                ),
        )
        .expect("register F12");

    // Define a layer that shadows Ctrl+C
    let vim_layer = Layer::new("vim-normal")
        .bind(
            Hotkey::new(Key::C).modifier(Modifier::Ctrl),
            Action::from(|| {}),
        )
        .bind(Hotkey::new(Key::D), Action::from(|| {}))
        .description("Vim normal mode");
    matcher.define_layer(vim_layer).expect("define vim-normal");

    (matcher, copy_id)
}
