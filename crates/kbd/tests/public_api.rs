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
use kbd::binding::Binding;
use kbd::binding::BindingId;
use kbd::binding::BindingOptions;
use kbd::binding::BindingSource;
use kbd::binding::OverlayVisibility;
use kbd::dispatcher::Dispatcher;
use kbd::dispatcher::MatchResult;
use kbd::error::LayerError;
use kbd::error::ParseHotkeyError;
use kbd::error::RegisterError;
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
            Binding::new(BindingId::new(), Hotkey::new(Key::A), Action::Suppress)
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
    let _ = dispatcher.pending_timeouts();

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
        Err(RegisterError::AlreadyRegistered)
    ));

    // LayerNotDefined
    assert!(matches!(
        dispatcher.push_layer("nope"),
        Err(LayerError::NotDefined)
    ));

    // EmptyLayerStack
    assert!(matches!(
        dispatcher.pop_layer(),
        Err(LayerError::EmptyStack)
    ));

    // LayerAlreadyDefined
    dispatcher
        .define_layer(Layer::new("x").bind(Key::A, Action::Suppress).unwrap())
        .unwrap();
    assert!(matches!(
        dispatcher.define_layer(Layer::new("x").bind(Key::B, Action::Suppress).unwrap()),
        Err(LayerError::AlreadyDefined)
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
        .register_binding(Binding::new(BindingId::new(), hotkey, Action::Suppress))
        .unwrap();

    let result =
        dispatcher.register_binding(Binding::new(BindingId::new(), hotkey, Action::Suppress));
    assert!(matches!(result, Err(RegisterError::AlreadyRegistered)));
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
    assert!(matches!(result, Err(RegisterError::AlreadyRegistered)));
}

mod observations {
    use kbd::device::DeviceContext;
    use kbd::device::DeviceFilter;
    use kbd::device::DeviceInfo;
    use kbd::hotkey::ModifierSet;
    use kbd::observation::BindingPattern;
    use kbd::observation::KeyboardObservation;
    use kbd::observation::LogicalKeyValue;
    use kbd::policy::RepeatPolicy;
    use kbd::tap_hold::TapHoldOptions;

    use super::*;

    fn logical(text: &str) -> BindingPattern {
        BindingPattern::logical(LogicalKeyValue::Character(text.into()), ModifierSet::NONE)
    }

    fn event(physical: Option<Key>, text: &str) -> KeyboardObservation {
        KeyboardObservation {
            physical,
            logical: Some(LogicalKeyValue::Character(text.into()).into()),
            modifiers: ModifierSet::NONE,
            modifier_observation: None,
            transition: KeyTransition::Press,
        }
    }

    fn emitted(result: MatchResult<'_>) -> Key {
        match result {
            MatchResult::Matched {
                action: Action::EmitHotkey(hotkey),
                ..
            } => hotkey.key(),
            other => panic!("expected emitted hotkey, got {other:?}"),
        }
    }

    fn emit(key: Key) -> Action {
        Action::EmitHotkey(Hotkey::new(key))
    }

    #[test]
    fn asymmetric_identities_match_only_their_own_domain() {
        let observed = event(Some(Key::Q), "a");
        let mut dispatcher = Dispatcher::new();
        dispatcher.register("A", emit(Key::X)).unwrap();
        assert!(matches!(
            dispatcher.process_event(&observed),
            MatchResult::NoMatch
        ));
        let id = dispatcher
            .register_pattern(logical("a"), emit(Key::Y), BindingOptions::default())
            .unwrap();
        assert_eq!(emitted(dispatcher.process_event(&observed)), Key::Y);
        assert_eq!(emitted(dispatcher.process_event(&event(None, "a"))), Key::Y);
        assert!(dispatcher.bindings_for_key(Hotkey::new(Key::Q)).is_none());
        assert_eq!(
            dispatcher.bindings_for_event(&observed).unwrap().pattern(),
            &logical("a")
        );
        assert_eq!(
            dispatcher
                .bindings_for_pattern(&logical("a"))
                .unwrap()
                .pattern(),
            &logical("a")
        );
        assert!(dispatcher.is_pattern_registered(&logical("a")));
        dispatcher.unregister(id);
        assert!(!dispatcher.is_pattern_registered(&logical("a")));
        assert!(matches!(
            dispatcher.process_event(&observed),
            MatchResult::NoMatch
        ));

        let legacy = KeyboardObservation::from_hotkey(Hotkey::new(Key::Q), KeyTransition::Release);
        assert_eq!(legacy.physical, Some(Key::Q));
        assert_eq!(legacy.logical, None);
        assert_eq!(legacy.transition, KeyTransition::Release);
        let binding = Binding::new(BindingId::new(), logical("a"), Action::Suppress);
        assert_eq!(binding.hotkey(), None);
        assert_eq!(binding.pattern(), &logical("a"));
    }

    #[test]
    fn logical_unicode_is_exact_and_named_keys_are_not_character_strings() {
        let mut dispatcher = Dispatcher::new();
        for text in ["é", "👩‍💻", "a"] {
            dispatcher
                .register_pattern(logical(text), emit(Key::Y), BindingOptions::default())
                .unwrap();
            assert_eq!(
                emitted(dispatcher.process_event(&event(None, text))),
                Key::Y
            );
        }
        for text in ["e\u{301}", "É", "👩", "A", "aa"] {
            assert!(
                matches!(
                    dispatcher.process_event(&event(None, text)),
                    MatchResult::NoMatch
                ),
                "{text}"
            );
        }
        let named = BindingPattern::logical(kbd::observation::NamedKey::Enter, ModifierSet::CTRL);
        dispatcher
            .register_pattern(named.clone(), emit(Key::Z), BindingOptions::default())
            .unwrap();
        let mut observed = event(None, "Enter");
        observed.modifiers = ModifierSet::CTRL;
        assert!(matches!(
            dispatcher.process_event(&observed),
            MatchResult::NoMatch
        ));
        observed.logical = Some(kbd::observation::NamedKey::Enter.into());
        assert_eq!(emitted(dispatcher.process_event(&observed)), Key::Z);
        observed.modifiers = ModifierSet::CTRL.with(Modifier::Shift);
        assert!(matches!(
            dispatcher.process_event(&observed),
            MatchResult::NoMatch
        ));
        observed.modifiers = ModifierSet::NONE;
        assert!(matches!(
            dispatcher.process_event(&observed),
            MatchResult::NoMatch
        ));
        assert_eq!(
            dispatcher
                .list_bindings()
                .iter()
                .filter(|b| b.pattern() == &named)
                .count(),
            1
        );
    }

    #[test]
    fn physical_tie_break_returns_one_callback_independent_of_registration_order() {
        for physical_first in [false, true] {
            let mut dispatcher = Dispatcher::new();
            let calls = Arc::new(AtomicUsize::new(0));
            let mut patterns = [(logical("a"), 100), (Hotkey::new(Key::Q).into(), 1)];
            if physical_first {
                patterns.reverse();
            }
            for (pattern, increment) in patterns {
                let calls = Arc::clone(&calls);
                dispatcher
                    .register_pattern(
                        pattern,
                        move || {
                            calls.fetch_add(increment, Ordering::Relaxed);
                        },
                        BindingOptions::default(),
                    )
                    .unwrap();
            }
            let observed = event(Some(Key::Q), "a");
            assert_eq!(
                dispatcher.bindings_for_event(&observed).unwrap().pattern(),
                &BindingPattern::Physical(Hotkey::new(Key::Q))
            );
            let MatchResult::Matched {
                action: Action::Callback(callback),
                ..
            } = dispatcher.process_event(&observed)
            else {
                panic!("expected callback");
            };
            callback();
            assert_eq!(calls.load(Ordering::Relaxed), 1);
            assert!(
                dispatcher.conflicts().is_empty(),
                "single-domain introspection must not infer a layout"
            );
        }
    }

    #[test]
    fn logical_source_and_device_priority_beat_physical_domain_priority() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register_with_options(
                Key::Q,
                emit(Key::X),
                BindingOptions::default().with_source("default"),
            )
            .unwrap();
        dispatcher
            .register_pattern(
                logical("a"),
                emit(Key::Y),
                BindingOptions::default().with_source("user"),
            )
            .unwrap();
        let observed = event(Some(Key::Q), "a");
        assert_eq!(emitted(dispatcher.process_event(&observed)), Key::Y);
        assert_eq!(
            dispatcher.bindings_for_event(&observed).unwrap().pattern(),
            &logical("a")
        );

        dispatcher
            .register_with_options(
                Key::Q,
                emit(Key::X),
                BindingOptions::default().with_source("user"),
            )
            .unwrap();
        assert_eq!(emitted(dispatcher.process_event(&observed)), Key::X);

        let device_pattern =
            BindingPattern::logical(LogicalKeyValue::Character("a".into()), ModifierSet::CTRL);
        dispatcher
            .register_pattern(
                device_pattern.clone(),
                emit(Key::Z),
                BindingOptions::default()
                    .with_source("default")
                    .with_device(DeviceFilter::name_contains("pad")),
            )
            .unwrap();
        let info = DeviceInfo::new("pad", 1, 2);
        let device = DeviceContext::new(7, &info).with_device_modifiers(ModifierSet::CTRL);
        assert_eq!(
            emitted(dispatcher.process_event_with_device(&observed, &device)),
            Key::Z
        );
        assert_eq!(
            dispatcher
                .bindings_for_event_with_device(&observed, &device)
                .unwrap()
                .pattern(),
            &device_pattern
        );
        let other = DeviceInfo::new("keyboard", 1, 3);
        let other_device = DeviceContext::new(8, &other).with_device_modifiers(ModifierSet::CTRL);
        assert_eq!(
            emitted(dispatcher.process_event_with_device(&observed, &other_device)),
            Key::X
        );
        assert_eq!(
            emitted(dispatcher.process_event_with_device(&event(None, "a"), &device)),
            Key::Z
        );
    }

    #[test]
    fn layer_scope_wins_across_domains_in_both_directions() {
        for logical_on_top in [false, true] {
            let mut dispatcher = Dispatcher::new();
            let physical = BindingPattern::Physical(Hotkey::new(Key::Q));
            let (top, bottom) = if logical_on_top {
                (logical("a"), physical)
            } else {
                (physical, logical("a"))
            };
            dispatcher
                .register_pattern(
                    bottom.clone(),
                    emit(Key::X),
                    BindingOptions::default().with_source("user"),
                )
                .unwrap();
            dispatcher
                .define_layer(Layer::new("bottom").bind_pattern(
                    bottom,
                    emit(Key::Y),
                    BindingOptions::default(),
                ))
                .unwrap();
            dispatcher
                .define_layer(Layer::new("top").bind_pattern(
                    top.clone(),
                    emit(Key::Z),
                    BindingOptions::default().with_source("default"),
                ))
                .unwrap();
            dispatcher.push_layer("bottom").unwrap();
            dispatcher.push_layer("top").unwrap();
            let observed = event(Some(Key::Q), "a");
            assert_eq!(
                dispatcher.bindings_for_event(&observed).unwrap().pattern(),
                &top
            );
            assert_eq!(emitted(dispatcher.process_event(&observed)), Key::Z);
        }
    }

    #[test]
    fn a_dual_identity_event_ticks_oneshot_once_and_ignores_release_repeat() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .define_layer(
                Layer::new("once")
                    .bind_pattern(
                        logical("a"),
                        emit(Key::Y),
                        BindingOptions::default().with_repeat_policy(RepeatPolicy::Allow),
                    )
                    .bind(Key::Q, emit(Key::X))
                    .unwrap()
                    .oneshot(2),
            )
            .unwrap();
        dispatcher.push_layer("once").unwrap();
        let mut observed = event(Some(Key::Q), "a");
        assert_eq!(emitted(dispatcher.process_event(&observed)), Key::X);
        assert_eq!(dispatcher.active_layers().len(), 1);
        observed.physical = None;
        for transition in [KeyTransition::Release, KeyTransition::Repeat] {
            observed.transition = transition;
            assert!(matches!(
                dispatcher.process_event(&observed),
                MatchResult::Ignored
            ));
            assert!(dispatcher.bindings_for_event(&observed).is_none());
            assert_eq!(dispatcher.active_layers().len(), 1);
        }
        observed.transition = KeyTransition::Press;
        assert_eq!(emitted(dispatcher.process_event(&observed)), Key::Y);
        assert!(dispatcher.active_layers().is_empty());
    }

    #[test]
    fn dual_identity_layer_effect_and_throttle_apply_once() {
        let mut dispatcher = Dispatcher::new();
        dispatcher.define_layer(Layer::new("mode")).unwrap();
        for pattern in [logical("a"), Hotkey::new(Key::Q).into()] {
            dispatcher
                .register_pattern(
                    pattern,
                    Action::ToggleLayer("mode".into()),
                    BindingOptions::default().with_debounce(Duration::from_secs(60)),
                )
                .unwrap();
        }
        let observed = event(Some(Key::Q), "a");
        assert!(matches!(
            dispatcher.process_event(&observed),
            MatchResult::Matched { .. }
        ));
        assert_eq!(dispatcher.active_layers().len(), 1);
        assert!(matches!(
            dispatcher.process_event(&observed),
            MatchResult::Throttled { .. }
        ));
        assert_eq!(dispatcher.active_layers().len(), 1);
        // The losing logical binding was not fired or throttled by the dual event.
        assert!(matches!(
            dispatcher.process_event(&event(None, "a")),
            MatchResult::Matched { .. }
        ));
        assert!(dispatcher.active_layers().is_empty());
    }

    #[test]
    fn physical_sequences_precede_logical_immediate_and_logical_only_press_mismatches() {
        for in_layer in [false, true] {
            let mut dispatcher = Dispatcher::new();
            if in_layer {
                dispatcher
                    .define_layer(
                        Layer::new("mode")
                            .bind_sequence("Q, X", emit(Key::Z))
                            .unwrap()
                            .bind_pattern(logical("a"), emit(Key::Y), BindingOptions::default()),
                    )
                    .unwrap();
                dispatcher.push_layer("mode").unwrap();
            } else {
                dispatcher.register_sequence("Q, X", emit(Key::Z)).unwrap();
                dispatcher
                    .register_pattern(logical("a"), emit(Key::Y), BindingOptions::default())
                    .unwrap();
            }
            let prefix = event(Some(Key::Q), "a");
            assert!(dispatcher.bindings_for_event(&prefix).is_none());
            assert!(matches!(
                dispatcher.process_event(&prefix),
                MatchResult::Pending {
                    steps_matched: 1,
                    ..
                }
            ));
            assert_eq!(
                emitted(dispatcher.process_event(&event(Some(Key::X), "unrelated"))),
                Key::Z
            );

            assert!(matches!(
                dispatcher.process_event(&prefix),
                MatchResult::Pending { .. }
            ));
            let mut logical_only = event(None, "a");
            logical_only.transition = KeyTransition::Release;
            assert!(matches!(
                dispatcher.process_event(&logical_only),
                MatchResult::Ignored
            ));
            assert!(dispatcher.pending_sequence().is_some());
            logical_only.transition = KeyTransition::Press;
            assert_eq!(emitted(dispatcher.process_event(&logical_only)), Key::Y);
            assert!(dispatcher.pending_sequence().is_none());
            assert!(matches!(
                dispatcher.process_event(&event(Some(Key::X), "x")),
                MatchResult::NoMatch
            ));
            // A logical spelling of the first physical key cannot start a sequence.
            assert!(matches!(
                dispatcher.process_event(&event(None, "q")),
                MatchResult::NoMatch
            ));
            assert!(dispatcher.pending_sequence().is_none());
        }
    }

    #[test]
    fn logical_only_press_interrupts_but_never_enrolls_physical_tap_hold() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register_tap_hold(
                Key::Q,
                emit(Key::X),
                emit(Key::Z),
                TapHoldOptions::new().with_threshold(Duration::from_secs(60)),
            )
            .unwrap();
        dispatcher
            .register_pattern(logical("q"), emit(Key::Y), BindingOptions::default())
            .unwrap();
        assert_eq!(emitted(dispatcher.process_event(&event(None, "q"))), Key::Y);
        assert!(dispatcher.pending_timeouts().is_empty());
        let mut physical = event(Some(Key::Q), "a");
        assert!(matches!(
            dispatcher.process_event(&physical),
            MatchResult::Matched {
                action: Action::Suppress,
                ..
            }
        ));
        physical.transition = KeyTransition::Release;
        assert_eq!(emitted(dispatcher.process_event(&physical)), Key::X);

        physical.transition = KeyTransition::Press;
        dispatcher.process_event(&physical);
        let mut logical_only = event(None, "q");
        for transition in [KeyTransition::Release, KeyTransition::Repeat] {
            logical_only.transition = transition;
            assert!(matches!(
                dispatcher.process_event(&logical_only),
                MatchResult::Ignored
            ));
            assert!(dispatcher.pending_timeouts().is_empty());
        }
        logical_only.transition = KeyTransition::Press;
        assert_eq!(emitted(dispatcher.process_event(&logical_only)), Key::Y);
        let pending = dispatcher.pending_timeouts();
        assert_eq!(pending.len(), 1);
        assert_eq!(
            emitted(dispatcher.match_pending_timeout(&pending[0]).unwrap()),
            Key::Z
        );
        assert!(dispatcher.pending_timeouts().is_empty());
    }

    #[test]
    fn logical_registration_and_introspection_keep_pattern_scope() {
        let mut dispatcher = Dispatcher::new();
        let pattern = logical("é");
        let default = dispatcher
            .register_pattern(
                pattern.clone(),
                emit(Key::X),
                BindingOptions::default().with_source("default"),
            )
            .unwrap();
        let user = dispatcher
            .register_pattern(
                pattern.clone(),
                emit(Key::Y),
                BindingOptions::default().with_source("user"),
            )
            .unwrap();
        assert!(matches!(
            dispatcher.register_pattern(
                pattern.clone(),
                Action::Suppress,
                BindingOptions::default().with_source("user")
            ),
            Err(RegisterError::AlreadyRegistered)
        ));
        assert_eq!(dispatcher.conflicts().len(), 1);
        assert_eq!(dispatcher.conflicts()[0].pattern(), &pattern);
        dispatcher.unregister(user);
        assert_eq!(emitted(dispatcher.process_event(&event(None, "é"))), Key::X);
        dispatcher
            .define_layer(Layer::new("swallow").swallow())
            .unwrap();
        dispatcher.push_layer("swallow").unwrap();
        assert!(matches!(
            dispatcher.process_event(&event(None, "é")),
            MatchResult::Suppressed
        ));
        assert!(dispatcher.bindings_for_pattern(&pattern).is_none());
        assert!(matches!(
            dispatcher.list_bindings()[0].shadowed,
            ShadowedStatus::SuppressedBy(_)
        ));
        dispatcher.unregister(default);
        assert!(dispatcher.list_bindings().is_empty());
    }
}
