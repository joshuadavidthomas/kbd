//! Integration tests that exercise kbd's public API as an outside consumer would.
//!
//! These complement the unit tests in each module by verifying that:
//! - Import paths work as documented
//! - Types compose correctly across module boundaries
//! - Real workflows (register → process → introspect) hold together

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use kbd::action::Action;
use kbd::binding::BindingId;
use kbd::binding::BindingOptions;
use kbd::binding::BindingSource;
use kbd::binding::OverlayVisibility;
use kbd::binding::RegisteredBinding;
use kbd::dispatcher::Dispatcher;
use kbd::dispatcher::MatchResult;
use kbd::error::Error;
use kbd::error::ParseHotkeyError;
use kbd::hotkey::Hotkey;
use kbd::hotkey::HotkeySequence;
use kbd::hotkey::Modifier;
use kbd::introspection::BindingLocation;
use kbd::introspection::ShadowedStatus;
use kbd::key::Key;
use kbd::key_state::KeyState;
use kbd::key_state::KeyTransition;
use kbd::layer::Layer;
use kbd::layer::LayerName;
use kbd::policy::KeyPropagation;

// Register, match, fire callback
#[test]
fn register_and_fire_callback() {
    let mut dispatcher = Dispatcher::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let cc = Arc::clone(&counter);

    dispatcher
        .register(Hotkey::new(Key::S).modifier(Modifier::Ctrl), move || {
            cc.fetch_add(1, Ordering::Relaxed);
        })
        .unwrap();

    let result = dispatcher.process(
        Hotkey::new(Key::S).modifier(Modifier::Ctrl),
        KeyTransition::Press,
    );
    if let MatchResult::Matched {
        action: Action::Callback(cb),
        ..
    } = result
    {
        cb();
    }
    assert_eq!(counter.load(Ordering::Relaxed), 1);
}

// Parse hotkey from string, register, process
#[test]
fn string_parsed_hotkey_matches() {
    let mut dispatcher = Dispatcher::new();
    let hotkey: Hotkey = "Ctrl+Shift+A".parse().unwrap();

    dispatcher.register(hotkey, Action::Suppress).unwrap();

    let result = dispatcher.process(hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::Matched { .. }));

    // Same hotkey built programmatically
    let built = Hotkey::new(Key::A)
        .modifier(Modifier::Ctrl)
        .modifier(Modifier::Shift);
    let result = dispatcher.process(built, KeyTransition::Press);
    assert!(matches!(result, MatchResult::Matched { .. }));
}

// Full layer lifecycle: define, push, match, pop, no-match
#[test]
fn layer_lifecycle() {
    let mut dispatcher = Dispatcher::new();

    dispatcher
        .define_layer(
            Layer::new("vim-normal")
                .bind(Key::H, Action::Suppress)
                .unwrap()
                .bind(Key::J, Action::Suppress)
                .unwrap()
                .bind(Key::K, Action::Suppress)
                .unwrap()
                .bind(Key::L, Action::Suppress)
                .unwrap()
                .bind(Key::ESCAPE, Action::PopLayer)
                .unwrap()
                .description("Vim normal mode"),
        )
        .unwrap();

    // Not active yet
    let result = dispatcher.process(Hotkey::new(Key::H), KeyTransition::Press);
    assert!(matches!(result, MatchResult::NoMatch));

    // Push and match
    dispatcher.push_layer("vim-normal").unwrap();
    let result = dispatcher.process(Hotkey::new(Key::H), KeyTransition::Press);
    assert!(matches!(result, MatchResult::Matched { .. }));

    // Pop via action
    dispatcher.process(Hotkey::new(Key::ESCAPE), KeyTransition::Press);

    // No longer active
    let result = dispatcher.process(Hotkey::new(Key::H), KeyTransition::Press);
    assert!(matches!(result, MatchResult::NoMatch));
}

// Layer shadows global binding, global falls through for non-overlapping keys
#[test]
fn layer_shadows_global_and_falls_through() {
    let mut dispatcher = Dispatcher::new();
    let global_counter = Arc::new(AtomicUsize::new(0));
    let gc = Arc::clone(&global_counter);
    let layer_counter = Arc::new(AtomicUsize::new(0));
    let lc = Arc::clone(&layer_counter);

    // Global Ctrl+C
    dispatcher
        .register(
            Hotkey::new(Key::C).modifier(Modifier::Ctrl),
            Action::from(move || {
                gc.fetch_add(1, Ordering::Relaxed);
            }),
        )
        .unwrap();

    // Layer also binds Ctrl+C (shadows) plus H
    dispatcher
        .define_layer(
            Layer::new("nav")
                .bind(
                    Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                    Action::from(move || {
                        lc.fetch_add(1, Ordering::Relaxed);
                    }),
                )
                .unwrap()
                .bind(Key::H, Action::Suppress)
                .unwrap(),
        )
        .unwrap();
    dispatcher.push_layer("nav").unwrap();

    // Ctrl+C fires layer version, not global
    let result = dispatcher.process(
        Hotkey::new(Key::C).modifier(Modifier::Ctrl),
        KeyTransition::Press,
    );
    if let MatchResult::Matched {
        action: Action::Callback(cb),
        ..
    } = result
    {
        cb();
    }
    assert_eq!(layer_counter.load(Ordering::Relaxed), 1);
    assert_eq!(global_counter.load(Ordering::Relaxed), 0);
}

// Swallow layer blocks unmatched keys from reaching globals
#[test]
fn swallow_layer_blocks_globals() {
    let mut dispatcher = Dispatcher::new();

    dispatcher
        .register(Hotkey::new(Key::X), Action::Suppress)
        .unwrap();
    dispatcher
        .define_layer(
            Layer::new("modal")
                .bind(Key::H, Action::Suppress)
                .unwrap()
                .swallow(),
        )
        .unwrap();
    dispatcher.push_layer("modal").unwrap();

    // H matches in the swallow layer
    let result = dispatcher.process(Hotkey::new(Key::H), KeyTransition::Press);
    assert!(matches!(result, MatchResult::Matched { .. }));

    // X is blocked (suppressed) — doesn't fall through to global
    let result = dispatcher.process(Hotkey::new(Key::X), KeyTransition::Press);
    assert!(matches!(result, MatchResult::Suppressed));
}

// KeyPropagation::Continue is returned in MatchResult
#[test]
fn propagation_continue_returned_in_match() {
    let mut dispatcher = Dispatcher::new();

    dispatcher
        .register_binding(
            RegisteredBinding::new(BindingId::new(), Hotkey::new(Key::A), Action::Suppress)
                .with_propagation(KeyPropagation::Continue),
        )
        .unwrap();

    let result = dispatcher.process(Hotkey::new(Key::A), KeyTransition::Press);
    match result {
        MatchResult::Matched { propagation, .. } => {
            assert_eq!(propagation, KeyPropagation::Continue);
        }
        other => panic!("expected Matched, got {other:?}"),
    }
}

// Toggle layer on and off via Action::ToggleLayer through process()
#[test]
fn toggle_layer_via_action() {
    let mut dispatcher = Dispatcher::new();

    dispatcher
        .define_layer(Layer::new("nav").bind(Key::H, Action::Suppress).unwrap())
        .unwrap();
    dispatcher
        .register(
            Hotkey::new(Key::F2),
            Action::ToggleLayer(LayerName::from("nav")),
        )
        .unwrap();

    // Toggle on
    dispatcher.process(Hotkey::new(Key::F2), KeyTransition::Press);
    assert_eq!(dispatcher.active_layers().len(), 1);

    // Toggle off
    dispatcher.process(Hotkey::new(Key::F2), KeyTransition::Press);
    assert!(dispatcher.active_layers().is_empty());
}

// Oneshot layer auto-pops after the configured number of keypresses
#[test]
fn oneshot_auto_pops() {
    let mut dispatcher = Dispatcher::new();

    dispatcher
        .define_layer(
            Layer::new("oneshot")
                .bind(Key::H, Action::Suppress)
                .unwrap()
                .bind(Key::J, Action::Suppress)
                .unwrap()
                .oneshot(2),
        )
        .unwrap();
    dispatcher.push_layer("oneshot").unwrap();
    assert_eq!(dispatcher.active_layers().len(), 1);

    // First keypress — still active
    dispatcher.process(Hotkey::new(Key::H), KeyTransition::Press);
    assert_eq!(dispatcher.active_layers().len(), 1);

    // Second keypress — auto-pops
    dispatcher.process(Hotkey::new(Key::J), KeyTransition::Press);
    assert!(dispatcher.active_layers().is_empty());
}

// Timeout layer auto-pops after inactivity
#[test]
fn timeout_auto_pops() {
    let mut dispatcher = Dispatcher::new();

    dispatcher
        .define_layer(
            Layer::new("timed")
                .bind(Key::H, Action::Suppress)
                .unwrap()
                .timeout(Duration::from_millis(50)),
        )
        .unwrap();
    dispatcher.push_layer("timed").unwrap();

    assert_eq!(dispatcher.active_layers().len(), 1);

    std::thread::sleep(Duration::from_millis(80));
    let _ = dispatcher.check_timeouts();

    assert!(dispatcher.active_layers().is_empty());
}

// Introspection: list_bindings reflects global + layer state
#[test]
fn introspection_full_picture() {
    let mut dispatcher = Dispatcher::new();

    dispatcher
        .register_with_options(
            Hotkey::new(Key::C).modifier(Modifier::Ctrl),
            Action::Suppress,
            BindingOptions::default()
                .with_description("Copy")
                .with_source(BindingSource::new("user"))
                .with_overlay_visibility(OverlayVisibility::Hidden),
        )
        .unwrap();

    dispatcher
        .define_layer(
            Layer::new("nav")
                .bind_with_options(
                    Key::H,
                    Action::Suppress,
                    BindingOptions::default()
                        .with_description("Move left")
                        .with_source(BindingSource::new("plugin"))
                        .with_overlay_visibility(OverlayVisibility::Hidden),
                )
                .unwrap()
                .bind(
                    Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                    Action::Suppress,
                )
                .unwrap()
                .description("Navigation layer"),
        )
        .unwrap();

    // Before pushing: layer bindings are inactive, no conflicts
    let bindings = dispatcher.list_bindings();
    assert!(bindings.len() >= 3); // 1 global + 2 layer
    assert!(dispatcher.conflicts().is_empty());

    // Push layer: Ctrl+C is now shadowed
    dispatcher.push_layer("nav").unwrap();
    let conflicts = dispatcher.conflicts();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(
        conflicts[0].hotkey,
        Hotkey::new(Key::C).modifier(Modifier::Ctrl)
    );

    // Active layers reflects stack
    let layers = dispatcher.active_layers();
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].name.as_str(), "nav");
    assert_eq!(layers[0].description.as_deref(), Some("Navigation layer"));
    assert_eq!(layers[0].binding_count, 2);

    // bindings_for_key resolves through layer stack
    let info = dispatcher
        .bindings_for_key(Hotkey::new(Key::C).modifier(Modifier::Ctrl))
        .unwrap();
    assert_eq!(
        info.location,
        BindingLocation::Layer(LayerName::from("nav"))
    );

    let layer_info = dispatcher.bindings_for_key(Hotkey::new(Key::H)).unwrap();
    assert_eq!(
        layer_info.source.as_ref().map(BindingSource::as_str),
        Some("plugin")
    );
    assert_eq!(layer_info.description.as_deref(), Some("Move left"));
    assert_eq!(layer_info.overlay_visibility, OverlayVisibility::Hidden);

    // Global binding is shadowed
    let all = dispatcher.list_bindings();
    let global_copy = all
        .iter()
        .find(|b| b.location == BindingLocation::Global)
        .unwrap();
    assert!(matches!(
        global_copy.shadowed,
        ShadowedStatus::ShadowedBy(_)
    ));
    assert_eq!(
        global_copy.source.as_ref().map(BindingSource::as_str),
        Some("user")
    );
    assert_eq!(global_copy.overlay_visibility, OverlayVisibility::Hidden);
}

// KeyState + Dispatcher integration: track modifier state, build hotkey, dispatch
#[test]
fn key_state_feeds_dispatcher() {
    let mut key_state = KeyState::default();
    let mut dispatcher = Dispatcher::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let cc = Arc::clone(&counter);

    dispatcher
        .register(Hotkey::new(Key::S).modifier(Modifier::Ctrl), move || {
            cc.fetch_add(1, Ordering::Relaxed);
        })
        .unwrap();

    // Simulate: user presses Ctrl, then S
    key_state.apply_device_event(0, Key::CONTROL_LEFT, KeyTransition::Press);

    // Build the hotkey the way a bridge crate would
    let modifiers = key_state.active_modifiers();
    let hotkey = Hotkey::with_modifiers(Key::S, modifiers);

    let result = dispatcher.process(hotkey, KeyTransition::Press);
    if let MatchResult::Matched {
        action: Action::Callback(cb),
        ..
    } = result
    {
        cb();
    }
    assert_eq!(counter.load(Ordering::Relaxed), 1);
}

// Error types are accessible and correct
#[test]
fn error_variants_accessible() {
    let mut dispatcher = Dispatcher::new();

    // AlreadyRegistered
    dispatcher
        .register(Hotkey::new(Key::A), Action::Suppress)
        .unwrap();
    assert!(matches!(
        dispatcher.register(Hotkey::new(Key::A), Action::Suppress),
        Err(Error::AlreadyRegistered)
    ));

    // LayerNotDefined
    assert!(matches!(
        dispatcher.push_layer("nope"),
        Err(Error::LayerNotDefined)
    ));

    // EmptyLayerStack
    assert!(matches!(
        dispatcher.pop_layer(),
        Err(Error::EmptyLayerStack)
    ));

    // LayerAlreadyDefined
    dispatcher
        .define_layer(Layer::new("x").bind(Key::A, Action::Suppress).unwrap())
        .unwrap();
    assert!(matches!(
        dispatcher.define_layer(Layer::new("x").bind(Key::B, Action::Suppress).unwrap()),
        Err(Error::LayerAlreadyDefined)
    ));
}

// Parse error types
#[test]
fn parse_hotkey_errors() {
    let result = "".parse::<Hotkey>();
    assert!(matches!(result, Err(ParseHotkeyError::Empty)));

    let result = "Ctrl+".parse::<Hotkey>();
    assert!(matches!(result, Err(ParseHotkeyError::EmptySegment)));

    let result = "Ctrl+NotAKey".parse::<Hotkey>();
    assert!(matches!(result, Err(ParseHotkeyError::UnknownToken(_))));
}

// HotkeySequence parsing and accessors
#[test]
fn hotkey_sequence_round_trip() {
    let seq: HotkeySequence = "Ctrl+K, Ctrl+C".parse().unwrap();
    assert_eq!(seq.steps().len(), 2);

    let display = seq.to_string();
    let reparsed: HotkeySequence = display.parse().unwrap();
    assert_eq!(reparsed.steps().len(), 2);
}

// Display/FromStr round-trip for hotkeys
#[test]
fn hotkey_display_parse_round_trip() {
    let hotkey = Hotkey::new(Key::S)
        .modifier(Modifier::Ctrl)
        .modifier(Modifier::Shift);
    let displayed = hotkey.to_string();
    let parsed: Hotkey = displayed.parse().unwrap();
    assert_eq!(parsed, hotkey);
}

// Key::as_str returns display name
#[test]
fn key_as_str() {
    assert_eq!(Key::A.as_str(), "A");
    assert_eq!(Key::ENTER.as_str(), "Enter");
    assert_eq!(Key::ESCAPE.as_str(), "Escape");
    assert_eq!(Key::ARROW_UP.as_str(), "Up");
    assert_eq!(Key::DIGIT0.as_str(), "0");
}

// Unregister removes a binding and it no longer matches
#[test]
fn unregister_stops_matching() {
    let mut dispatcher = Dispatcher::new();
    let id = dispatcher
        .register(Hotkey::new(Key::A), Action::Suppress)
        .unwrap();

    // Matches before unregister
    let result = dispatcher.process(Hotkey::new(Key::A), KeyTransition::Press);
    assert!(matches!(result, MatchResult::Matched { .. }));

    dispatcher.unregister(id);

    // No longer matches
    let result = dispatcher.process(Hotkey::new(Key::A), KeyTransition::Press);
    assert!(matches!(result, MatchResult::NoMatch));

    // Introspection also reflects removal
    assert!(dispatcher.list_bindings().is_empty());
}

// Dispatcher::default() works the same as Dispatcher::new()
#[test]
fn dispatcher_default_equals_new() {
    let d1 = Dispatcher::new();
    let d2 = Dispatcher::default();
    assert!(d1.list_bindings().is_empty());
    assert!(d2.list_bindings().is_empty());
}

// Non-press events are ignored
#[test]
fn release_and_repeat_ignored() {
    let mut dispatcher = Dispatcher::new();
    dispatcher
        .register(Hotkey::new(Key::A), Action::Suppress)
        .unwrap();

    assert!(matches!(
        dispatcher.process(Hotkey::new(Key::A), KeyTransition::Release),
        MatchResult::Ignored
    ));
    assert!(matches!(
        dispatcher.process(Hotkey::new(Key::A), KeyTransition::Repeat),
        MatchResult::Ignored
    ));
}

// Multiple layers stacked: topmost wins
#[test]
fn topmost_layer_wins() {
    let mut dispatcher = Dispatcher::new();
    let layer1_counter = Arc::new(AtomicUsize::new(0));
    let l1c = Arc::clone(&layer1_counter);
    let layer2_counter = Arc::new(AtomicUsize::new(0));
    let l2c = Arc::clone(&layer2_counter);

    dispatcher
        .define_layer(
            Layer::new("bottom")
                .bind(
                    Key::H,
                    Action::from(move || {
                        l1c.fetch_add(1, Ordering::Relaxed);
                    }),
                )
                .unwrap(),
        )
        .unwrap();
    dispatcher
        .define_layer(
            Layer::new("top")
                .bind(
                    Key::H,
                    Action::from(move || {
                        l2c.fetch_add(1, Ordering::Relaxed);
                    }),
                )
                .unwrap(),
        )
        .unwrap();

    dispatcher.push_layer("bottom").unwrap();
    dispatcher.push_layer("top").unwrap();

    let result = dispatcher.process(Hotkey::new(Key::H), KeyTransition::Press);
    if let MatchResult::Matched {
        action: Action::Callback(cb),
        ..
    } = result
    {
        cb();
    }
    assert_eq!(layer2_counter.load(Ordering::Relaxed), 1);
    assert_eq!(layer1_counter.load(Ordering::Relaxed), 0);
}

// Modifier::collect_active builds modifiers that dispatch correctly
#[test]
fn collect_active_builds_dispatchable_hotkey() {
    let mut dispatcher = Dispatcher::new();
    dispatcher
        .register(
            Hotkey::new(Key::S)
                .modifier(Modifier::Ctrl)
                .modifier(Modifier::Shift),
            Action::Suppress,
        )
        .unwrap();

    // Simulate what a bridge crate does: collect framework flags into modifiers
    let mods = Modifier::collect_active([
        (true, Modifier::Ctrl),
        (true, Modifier::Shift),
        (false, Modifier::Alt),
        (false, Modifier::Super),
    ]);
    let hotkey = Hotkey::with_modifiers(Key::S, mods);

    let result = dispatcher.process(hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::Matched { .. }));
}

// register_binding duplicate returns error
#[test]
fn register_binding_duplicate_returns_error() {
    let mut dispatcher = Dispatcher::new();
    let hotkey = Hotkey::new(Key::A);

    dispatcher
        .register_binding(RegisteredBinding::new(
            BindingId::new(),
            hotkey,
            Action::Suppress,
        ))
        .unwrap();

    let result = dispatcher.register_binding(RegisteredBinding::new(
        BindingId::new(),
        hotkey,
        Action::Suppress,
    ));
    assert!(matches!(result, Err(Error::AlreadyRegistered)));
}

#[test]
fn register_with_options_standard_tier_sources_still_conflict() {
    let mut dispatcher = Dispatcher::new();
    let hotkey = Hotkey::new(Key::A);

    dispatcher
        .register_with_options(
            hotkey,
            Action::Suppress,
            BindingOptions::default().with_source("plugin"),
        )
        .unwrap();

    let result = dispatcher.register(hotkey, Action::Suppress);
    assert!(matches!(result, Err(Error::AlreadyRegistered)));
}
