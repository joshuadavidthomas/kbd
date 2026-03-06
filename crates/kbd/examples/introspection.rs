//! Query dispatcher state — `list_bindings()`, `bindings_for_key()`,
//! `active_layers()`, `conflicts()`.
//!
//! Introspection lets you build help screens, hotkey overlays, and
//! keybinding editors. Immediate global bindings can carry metadata
//! (description, provenance, overlay visibility), and the dispatcher can tell
//! you what's active, what's shadowed, and what would fire for any given key.
//!
//! ```sh
//! cargo run -p kbd --example introspection
//! ```

use kbd::action::Action;
use kbd::binding::BindingId;
use kbd::binding::BindingOptions;
use kbd::binding::BindingSource;
use kbd::binding::OverlayVisibility;
use kbd::dispatcher::Dispatcher;
use kbd::hotkey::Hotkey;
use kbd::hotkey::Modifier;
use kbd::introspection::BindingInfo;
use kbd::introspection::BindingLocation;
use kbd::introspection::ShadowedStatus;
use kbd::key::Key;
use kbd::layer::Layer;

fn main() {
    let (mut dispatcher, copy_id) = setup_dispatcher();

    println!("=== Introspection demo ===");
    println!();

    // List all bindings
    println!("1. All registered bindings:");
    print_bindings(&dispatcher.list_bindings());
    println!();

    // Query what would fire for a specific key
    println!("2. What fires for Ctrl+C (no layers active)?");
    let hotkey = Hotkey::new(Key::C).modifier(Modifier::Ctrl);
    match dispatcher.bindings_for_key(&hotkey) {
        Some(info) => println!("  {}", format_binding(&info)),
        None => println!("  (nothing)"),
    }
    println!();

    // Push the vim layer — now Ctrl+C is shadowed
    dispatcher
        .push_layer("vim-normal")
        .expect("push vim-normal");

    println!("3. Active layers:");
    for layer in dispatcher.active_layers() {
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
    print_bindings(&dispatcher.list_bindings());
    println!();

    // What fires for Ctrl+C now?
    println!("5. What fires for Ctrl+C (with vim-normal layer)?");
    match dispatcher.bindings_for_key(&hotkey) {
        Some(info) => println!("  {}", format_binding(&info)),
        None => println!("  (nothing)"),
    }
    println!();

    // Show conflicts
    println!("6. Conflicts (shadowed bindings):");
    let conflicts = dispatcher.conflicts();
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

    // Filter for overlay-visible bindings only.
    println!("7. Overlay-visible bindings only:");
    let visible: Vec<_> = dispatcher
        .list_bindings()
        .into_iter()
        .filter(|b| b.overlay_visibility == OverlayVisibility::Visible)
        .collect();
    print_bindings(&visible);

    // Source-aware precedence — user bindings override defaults,
    // and unregistering a user binding promotes the default back.
    dispatcher.pop_layer().expect("pop vim-normal");

    println!();
    println!("8. Source-aware precedence (Ctrl+S):");
    let save_hotkey = Hotkey::new(Key::S).modifier(Modifier::Ctrl);

    // Right now Ctrl+S is a "default" source binding
    println!("  Before user override:");
    print_binding_for_key(&dispatcher, &save_hotkey);

    // Register a user override for the same hotkey
    let user_save_id = dispatcher
        .register_with_options(
            save_hotkey.clone(),
            Action::from(|| {}),
            BindingOptions::default()
                .with_description("Save file (auto-format first)")
                .with_source(BindingSource::new("user")),
        )
        .expect("register user Ctrl+S");

    // The user binding wins — the default is shadowed
    println!("  After user override:");
    print_binding_for_key(&dispatcher, &save_hotkey);

    // Show the conflict
    let save_conflicts: Vec<_> = dispatcher
        .conflicts()
        .into_iter()
        .filter(|c| c.hotkey == save_hotkey)
        .collect();
    for conflict in &save_conflicts {
        println!(
            "  Conflict: {} shadows {}",
            format_location(&conflict.shadowing_binding),
            format_location(&conflict.shadowed_binding),
        );
    }

    // Unregister the user binding — the default is promoted back
    dispatcher.unregister(user_save_id);
    println!("  After removing user override:");
    print_binding_for_key(&dispatcher, &save_hotkey);

    // Clean up — demonstrate that unregister works
    dispatcher.unregister(copy_id);
    println!();
    println!(
        "After unregistering global Ctrl+C: {} total bindings",
        dispatcher.list_bindings().len()
    );
}

fn print_binding_for_key(dispatcher: &Dispatcher, hotkey: &Hotkey) {
    match dispatcher.bindings_for_key(hotkey) {
        Some(info) => println!("    {}", format_binding(&info)),
        None => println!("    (nothing)"),
    }
}

fn print_bindings(bindings: &[BindingInfo]) {
    for binding in bindings {
        println!("  {}", format_binding(binding));
    }
}

fn format_binding(b: &BindingInfo) -> String {
    let desc = b.description.as_deref().unwrap_or("(no description)");
    let source = b
        .source
        .as_ref()
        .map_or(String::new(), |source| format!(" source={source}"));
    let shadow = match &b.shadowed {
        ShadowedStatus::Active => "active".to_string(),
        ShadowedStatus::ShadowedBy(name) => format!("shadowed by layer binding in {name}"),
        ShadowedStatus::SuppressedBy(name) => format!("suppressed by swallow layer {name}"),
        ShadowedStatus::ShadowedByGlobal => "shadowed by global override".to_string(),
        ShadowedStatus::ShadowedBySequence(location) => {
            format!(
                "shadowed by sequence in {}",
                format_location_from_location(location)
            )
        }
        ShadowedStatus::Inactive => "inactive".to_string(),
        _ => "unknown".to_string(),
    };
    let vis = match b.overlay_visibility {
        OverlayVisibility::Hidden => " [hidden]",
        _ => "",
    };
    format!(
        "{:20} {:30} [{}, {}{source}]{vis}",
        b.hotkey.to_string(),
        desc,
        format_location(b),
        shadow,
    )
}

fn format_location(b: &BindingInfo) -> String {
    format_location_from_location(&b.location)
}

fn format_location_from_location(location: &BindingLocation) -> String {
    match location {
        BindingLocation::Global => "global".to_string(),
        BindingLocation::Layer(name) => format!("layer:{name}"),
        _ => "unknown".to_string(),
    }
}

fn setup_dispatcher() -> (Dispatcher, BindingId) {
    let mut dispatcher = Dispatcher::new();

    // Register global bindings with metadata
    let copy_id = dispatcher
        .register_with_options(
            Hotkey::new(Key::C).modifier(Modifier::Ctrl),
            Action::from(|| {}),
            BindingOptions::default()
                .with_description("Copy to clipboard")
                .with_source(BindingSource::new("user")),
        )
        .expect("register Ctrl+C");

    dispatcher
        .register_with_options(
            Hotkey::new(Key::V).modifier(Modifier::Ctrl),
            Action::from(|| {}),
            BindingOptions::default().with_description("Paste from clipboard"),
        )
        .expect("register Ctrl+V");

    dispatcher
        .register_with_options(
            Hotkey::new(Key::S).modifier(Modifier::Ctrl),
            Action::from(|| {}),
            BindingOptions::default()
                .with_description("Save file")
                .with_source(BindingSource::new("default")),
        )
        .expect("register Ctrl+S");

    // A hidden binding — won't appear in overlay views
    dispatcher
        .register_with_options(
            Hotkey::new(Key::F12),
            Action::from(|| {}),
            BindingOptions::default()
                .with_description("Debug panel (internal)")
                .with_overlay_visibility(OverlayVisibility::Hidden),
        )
        .expect("register F12");

    // Define a layer that shadows Ctrl+C
    let vim_layer = Layer::new("vim-normal")
        .bind(
            Hotkey::new(Key::C).modifier(Modifier::Ctrl),
            Action::from(|| {}),
        )
        .unwrap()
        .bind_with_options(
            Hotkey::new(Key::D),
            Action::from(|| {}),
            BindingOptions::default()
                .with_description("Delete current selection")
                .with_source(BindingSource::new("plugin"))
                .with_overlay_visibility(OverlayVisibility::Hidden),
        )
        .unwrap()
        .description("Vim normal mode");
    dispatcher
        .define_layer(vim_layer)
        .expect("define vim-normal");

    (dispatcher, copy_id)
}
