//! Deterministic XKB evidence and integration with the existing dispatcher.

use kbd::action::Action;
use kbd::binding::BindingOptions;
use kbd::dispatcher::Dispatcher;
use kbd::dispatcher::MatchResult;
use kbd::hotkey::Hotkey;
use kbd::hotkey::Modifier;
use kbd::hotkey::ModifierSet;
use kbd::key::Key;
use kbd::key_state::KeyTransition;
use kbd::observation::BindingPattern;
use kbd::observation::KeyboardObservation;
use kbd::observation::LogicalKey;
use kbd::observation::LogicalKeyValue;
use kbd::observation::ModifierObservation;
use kbd::observation::ModifierState;
use kbd::observation::NamedKey;
use kbd::policy::LogicalMatchPolicy;
use kbd_xkb::ModifierMapping;
use kbd_xkb::enrich;
use kbd_xkb::evdev_keycode;
use kbd_xkb::xkb;

// These are real XKB masks specified by this fixture, not ModifierSet bits.
const SHIFT: u8 = 1;
const LOCK: u8 = 2;
const CONTROL: u8 = 4;
const ALT: u8 = 8;
const SUPER: u8 = 64;
const LEVEL3: u8 = 128;
const MAPPING: ModifierMapping<'_> = ModifierMapping {
    modifiers: &[
        (Modifier::Shift, SHIFT),
        (Modifier::Ctrl, CONTROL),
        (Modifier::Alt, ALT),
        (Modifier::Super, SUPER),
        (Modifier::AltGraph, LEVEL3),
    ],
    ignored: LOCK,
};

fn state() -> xkb::State {
    let context =
        xkb::Context::new(xkb::CONTEXT_NO_DEFAULT_INCLUDES | xkb::CONTEXT_NO_ENVIRONMENT_NAMES);
    let keymap = xkb::Keymap::new_from_string(
        &context,
        include_str!("fixtures/layouts.xkb").to_owned(),
        xkb::KEYMAP_FORMAT_TEXT_V1,
        xkb::KEYMAP_COMPILE_NO_FLAGS,
    )
    .expect("self-contained fixture compiles without installed XKB data");
    xkb::State::new(&keymap)
}

fn character(value: &str) -> LogicalKey {
    LogicalKeyValue::Character(value.to_owned()).into()
}

fn event(state: &xkb::State, code: u16, key: Key, modifiers: ModifierSet) -> KeyboardObservation {
    let mut event = KeyboardObservation::from_hotkey(
        Hotkey::with_modifiers(key, modifiers),
        KeyTransition::Press,
    );
    enrich(&mut event, state, evdev_keycode(code), MAPPING);
    event
}

fn select(state: &mut xkb::State, modifiers: u8, group: u32) {
    state.update_mask(u32::from(modifiers), 0, 0, 0, 0, group);
}

#[test]
fn groups_change_logical_identity_not_physical_facts() {
    let mut state = state();
    for (group, q, y, a) in [(0, "q", "y", "a"), (1, "a", "y", "q"), (2, "q", "z", "a")] {
        select(&mut state, 0, group);
        for (code, key, expected) in [(16, Key::Q, q), (21, Key::Y, y), (30, Key::A, a)] {
            let observed = event(&state, code, key, ModifierSet::CTRL);
            assert_eq!(observed.logical, Some(character(expected)));
            assert_eq!(observed.physical, Some(key));
            assert_eq!(observed.modifiers, ModifierSet::CTRL);
            assert_eq!(observed.physical_modifiers().active(), ModifierSet::CTRL);
            assert_eq!(
                observed.logical_modifiers().state.active(),
                ModifierSet::NONE
            );
            assert_eq!(state.key_get_layout(evdev_keycode(code)), group);
        }
        // Single-group keys use their own effective group, not the global one.
        assert_eq!(state.key_get_layout(evdev_keycode(2)), 0);
        assert_eq!(
            event(&state, 2, Key::DIGIT1, ModifierSet::NONE).logical,
            Some(character("1"))
        );
    }
    select(&mut state, SHIFT, 1);
    assert_eq!(
        event(&state, 16, Key::Q, ModifierSet::SHIFT).logical,
        Some(character("A"))
    );
}

#[test]
fn explicit_shift_is_not_removed_and_consumption_is_opt_in() {
    let mut state = state();
    select(&mut state, SHIFT, 0);
    let observed = event(&state, 2, Key::DIGIT1, ModifierSet::SHIFT);
    assert_eq!(observed.logical, Some(character("!")));
    assert_eq!(
        observed.logical_modifiers().state.active(),
        ModifierSet::SHIFT
    );
    assert_eq!(
        observed.logical_modifiers().consumed,
        Some(ModifierSet::SHIFT)
    );
    assert!(
        BindingPattern::Physical(Hotkey::with_modifiers(Key::DIGIT1, ModifierSet::SHIFT))
            .matches(&observed)
    );

    let bare = BindingPattern::logical(character("!"), ModifierSet::NONE);
    let explicit = BindingPattern::logical(character("!"), ModifierSet::SHIFT);
    assert!(!bare.matches(&observed));
    assert!(bare.matches_with_policy(&observed, LogicalMatchPolicy::Consumed));
    assert!(explicit.matches(&observed));
    assert!(explicit.matches_with_policy(&observed, LogicalMatchPolicy::Consumed));
    assert!(
        !BindingPattern::logical(character("!"), ModifierSet::CTRL)
            .matches_with_policy(&observed, LogicalMatchPolicy::Consumed)
    );

    select(&mut state, SHIFT | CONTROL | ALT, 0);
    let observed = event(&state, 2, Key::DIGIT1, ModifierSet::NONE);
    assert!(
        !BindingPattern::logical(character("!"), ModifierSet::CTRL)
            .matches_with_policy(&observed, LogicalMatchPolicy::Consumed)
    );
    select(&mut state, SHIFT | CONTROL, 0);
    let observed = event(&state, 2, Key::DIGIT1, ModifierSet::NONE);
    assert!(
        BindingPattern::logical(character("!"), ModifierSet::CTRL)
            .matches_with_policy(&observed, LogicalMatchPolicy::Consumed)
    );
    select(&mut state, 0, 0);
    let observed = event(&state, 2, Key::DIGIT1, ModifierSet::NONE);
    assert_eq!(observed.logical, Some(character("1")));
    assert_eq!(
        observed.logical_modifiers().consumed,
        Some(ModifierSet::SHIFT)
    );
    assert!(
        !BindingPattern::logical(character("1"), ModifierSet::SHIFT)
            .matches_with_policy(&observed, LogicalMatchPolicy::Consumed)
    );
}

#[test]
fn altgr_is_semantic_and_distinct_from_physical_alt() {
    let mut state = state();
    // Latched modifiers are effective even without a physically held key.
    state.update_mask(0, u32::from(LEVEL3), 0, 0, 0, 2);
    let observed = event(&state, 16, Key::Q, ModifierSet::ALT);
    assert_eq!(observed.logical, Some(character("@")));
    assert_eq!(observed.physical_modifiers().active(), ModifierSet::ALT);
    assert_eq!(observed.modifiers, ModifierSet::ALT);
    assert_eq!(
        observed.logical_modifiers().state.active(),
        ModifierSet::ALT_GRAPH
    );
    assert_eq!(
        observed.logical_modifiers().consumed,
        Some(ModifierSet::ALT_GRAPH.union(ModifierSet::SHIFT))
    );
    let at = BindingPattern::logical(character("@"), ModifierSet::NONE);
    assert!(!at.matches(&observed));
    assert!(at.matches_with_policy(&observed, LogicalMatchPolicy::Consumed));
    assert!(
        !BindingPattern::logical(character("@"), ModifierSet::ALT)
            .matches_with_policy(&observed, LogicalMatchPolicy::Consumed)
    );

    // The caller can explicitly assign a different semantic meaning to Mod5.
    let mut remapped = observed.clone();
    enrich(
        &mut remapped,
        &state,
        evdev_keycode(16),
        ModifierMapping {
            modifiers: &[(Modifier::Alt, LEVEL3)],
            ignored: LOCK,
        },
    );
    assert_eq!(
        remapped.logical_modifiers().state.active(),
        ModifierSet::ALT
    );
    assert_eq!(
        remapped.logical_modifiers().consumed,
        Some(ModifierSet::ALT)
    );
}

#[test]
fn identity_is_not_control_text_or_compose_output() {
    let mut state = state();
    select(&mut state, CONTROL, 0);
    assert_eq!(state.key_get_utf8(evdev_keycode(30)), "\u{1}");
    assert_eq!(
        event(&state, 30, Key::A, ModifierSet::CTRL).logical,
        Some(character("a"))
    );
    state.update_mask(0, 0, u32::from(LOCK), 0, 0, 0);
    let mut caps = event(&state, 30, Key::A, ModifierSet::NONE);
    let capital = BindingPattern::logical(character("A"), ModifierSet::NONE);
    assert_eq!(caps.logical, Some(character("A")));
    assert!(capital.matches(&caps)); // Lock is explicitly insignificant in MAPPING.
    enrich(
        &mut caps,
        &state,
        evdev_keycode(30),
        ModifierMapping {
            ignored: 0,
            ..MAPPING
        },
    );
    assert!(caps.logical_modifiers().state.extra_active());
    assert!(!capital.matches_with_policy(&caps, LogicalMatchPolicy::Consumed));
    select(&mut state, 0, 0);
    assert_eq!(
        event(&state, 192, Key::QUOTE, ModifierSet::NONE).logical,
        Some(NamedKey::Dead.into())
    );
    assert_eq!(
        event(&state, 30, Key::A, ModifierSet::NONE).logical,
        Some(character("a"))
    );
    assert_eq!(
        event(&state, 196, Key::Q, ModifierSet::NONE).logical,
        Some(character("🙂"))
    );
}

#[test]
fn shift_tab_and_keymap_preservation_have_explicit_policy() {
    let mut state = state();
    select(&mut state, SHIFT, 0);
    let tab = BindingPattern::logical(NamedKey::Tab, ModifierSet::NONE);
    let observed = event(&state, 15, Key::TAB, ModifierSet::SHIFT);
    assert_eq!(observed.logical, Some(NamedKey::Tab.into()));
    assert!(!tab.matches(&observed));
    assert!(tab.matches_with_policy(&observed, LogicalMatchPolicy::Consumed));
    let preserved = event(&state, 197, Key::TAB, ModifierSet::SHIFT);
    assert_eq!(
        preserved.logical_modifiers().consumed,
        Some(ModifierSet::NONE)
    );
    assert!(!tab.matches_with_policy(&preserved, LogicalMatchPolicy::Consumed));
}

#[test]
fn unknown_consumption_and_unknown_modifiers_are_not_known_empty() {
    let mut state = state();
    select(&mut state, SHIFT, 0);
    let valid = event(&state, 28, Key::ENTER, ModifierSet::SHIFT);
    assert_eq!(valid.logical_modifiers().consumed, Some(ModifierSet::NONE));
    let invalid = event(&state, 500, Key::ENTER, ModifierSet::SHIFT);
    assert_eq!(invalid.logical, None);
    assert_eq!(invalid.logical_modifiers().consumed, None);

    let mut observed = event(&state, 2, Key::DIGIT1, ModifierSet::SHIFT);
    observed
        .modifier_observation
        .as_mut()
        .unwrap()
        .logical
        .as_mut()
        .unwrap()
        .consumed = None;
    assert!(
        !BindingPattern::logical(character("!"), ModifierSet::NONE)
            .matches_with_policy(&observed, LogicalMatchPolicy::Consumed)
    );
    assert!(
        BindingPattern::logical(character("!"), ModifierSet::SHIFT)
            .matches_with_policy(&observed, LogicalMatchPolicy::Consumed)
    );

    select(&mut state, LEVEL3, 2);
    enrich(
        &mut observed,
        &state,
        evdev_keycode(16),
        ModifierMapping {
            modifiers: &[(Modifier::Shift, SHIFT)],
            ignored: LOCK,
        },
    );
    let semantic = observed.logical_modifiers();
    assert_eq!(semantic.state.known(), ModifierSet::SHIFT);
    assert!(semantic.state.extra_active());
    assert!(
        !BindingPattern::logical(character("@"), ModifierSet::NONE)
            .matches_with_policy(&observed, LogicalMatchPolicy::Consumed)
    );
    assert!(!semantic.state.known().contains(Modifier::Fn));
}

#[test]
fn multiple_real_bits_for_one_modifier_cannot_hide_an_unconsumed_bit() {
    let mut state = state();
    let mapping = ModifierMapping {
        modifiers: &[
            (Modifier::Shift, SHIFT),
            (Modifier::Shift, CONTROL),
            (Modifier::Fn, 0),
        ],
        ignored: LOCK,
    };
    for (active, expected) in [
        (SHIFT, ModifierSet::SHIFT),
        (SHIFT | CONTROL, ModifierSet::NONE),
    ] {
        select(&mut state, active, 0);
        let mut observed = event(&state, 2, Key::DIGIT1, ModifierSet::NONE);
        enrich(&mut observed, &state, evdev_keycode(2), mapping);
        assert_eq!(observed.logical_modifiers().consumed, Some(expected));
        assert_eq!(
            observed.logical_modifiers().state.active(),
            ModifierSet::SHIFT
        );
        assert!(
            observed
                .logical_modifiers()
                .state
                .known()
                .contains(Modifier::Fn)
        );
    }
}

#[test]
fn missing_unknown_and_multiple_symbols_clear_stale_logical_identity() {
    let state = state();
    assert_eq!(state.key_get_syms(evdev_keycode(194)).len(), 2);
    for code in [193, 194, 195, 500] {
        let mut observed = event(&state, 30, Key::A, ModifierSet::NONE);
        assert_eq!(observed.logical, Some(character("a")));
        enrich(&mut observed, &state, evdev_keycode(code), MAPPING);
        assert_eq!(observed.logical, None, "code {code}");
        assert_eq!(observed.physical, Some(Key::A));
    }
}

#[test]
fn named_and_keypad_symbols_do_not_become_control_characters() {
    let state = state();
    for (code, expected) in [
        (28, NamedKey::Enter),
        (42, NamedKey::Shift),
        (29, NamedKey::Control),
        (100, NamedKey::AltGraph),
        (125, NamedKey::Meta),
        (198, NamedKey::MediaPlayPause),
        (199, NamedKey::ArrowLeft),
        (201, NamedKey::F35),
    ] {
        assert_eq!(
            event(&state, code, Key::Q, ModifierSet::NONE).logical,
            Some(expected.into())
        );
    }
    assert_eq!(
        event(&state, 200, Key::Q, ModifierSet::NONE).logical,
        Some(character("1"))
    );
}

#[test]
fn keycode_boundary_and_logical_only_events_are_explicit() {
    assert_eq!(evdev_keycode(16).raw(), 24);
    assert_eq!(evdev_keycode(u16::MAX).raw(), 65_543);
    let state = state();
    let mut observed = event(&state, 30, Key::A, ModifierSet::NONE);
    observed.physical = None;
    enrich(&mut observed, &state, xkb::Keycode::new(24), MAPPING);
    assert_eq!(observed.logical, Some(character("q")));
    assert_eq!(observed.physical, None);
    // Passing an evdev code without conversion must not accidentally resolve Q.
    enrich(&mut observed, &state, xkb::Keycode::new(16), MAPPING);
    assert_eq!(observed.logical, None);
}

#[test]
fn caller_owns_updates_repeats_and_replacement() {
    let mut state = state();
    let shift = event(&state, 42, Key::SHIFT_LEFT, ModifierSet::NONE);
    assert_eq!(shift.logical, Some(NamedKey::Shift.into()));
    assert_eq!(state.serialize_mods(xkb::STATE_MODS_EFFECTIVE), 0);
    state.update_key(evdev_keycode(42), xkb::KeyDirection::Down);
    assert_eq!(
        state.serialize_mods(xkb::STATE_MODS_EFFECTIVE),
        u32::from(SHIFT)
    );
    let mut observed = event(&state, 2, Key::DIGIT1, ModifierSet::SHIFT);
    for transition in [
        KeyTransition::Press,
        KeyTransition::Repeat,
        KeyTransition::Repeat,
        KeyTransition::Release,
    ] {
        observed.transition = transition;
        enrich(&mut observed, &state, evdev_keycode(2), MAPPING);
        assert_eq!(observed.logical, Some(character("!")));
        assert_eq!(observed.transition, transition);
    }
    state.update_key(evdev_keycode(42), xkb::KeyDirection::Up);
    assert_eq!(state.serialize_mods(xkb::STATE_MODS_EFFECTIVE), 0);
    enrich(&mut observed, &state, evdev_keycode(2), MAPPING);
    assert_eq!(observed.logical, Some(character("1")));
    // No adapter state survives replacement of the caller's keymap/state.
    let mut replacement = self::state();
    select(&mut replacement, 0, 1);
    enrich(&mut observed, &replacement, evdev_keycode(16), MAPPING);
    assert_eq!(observed.logical, Some(character("a")));
}

#[test]
fn rich_physical_evidence_is_preserved_verbatim() {
    let state = state();
    let physical = ModifierState::new(ModifierSet::FN, ModifierSet::FN).with_extra_active(true);
    let mut observed =
        KeyboardObservation::from_hotkey(Hotkey::new(Key::Q), KeyTransition::Release);
    observed.modifier_observation = Some(ModifierObservation {
        physical,
        logical: None,
    });
    enrich(&mut observed, &state, evdev_keycode(16), MAPPING);
    assert_eq!(observed.physical_modifiers(), physical);
    assert_eq!(observed.modifiers, ModifierSet::NONE);
    assert_eq!(observed.transition, KeyTransition::Release);
    assert!(!observed.logical_modifiers().state.extra_active());
}

#[test]
fn one_observation_uses_one_dispatcher_and_consumption_requires_registration_opt_in() {
    let mut state = state();
    select(&mut state, SHIFT, 0);
    let observed = event(&state, 2, Key::DIGIT1, ModifierSet::SHIFT);
    let pattern = BindingPattern::logical(character("!"), ModifierSet::NONE);
    let mut dispatcher = Dispatcher::new();
    dispatcher
        .register_pattern(pattern.clone(), Action::Suppress, BindingOptions::default())
        .unwrap();
    assert!(matches!(
        dispatcher.process_event(&observed),
        MatchResult::NoMatch
    ));

    let mut dispatcher = Dispatcher::new();
    dispatcher
        .register_pattern(
            pattern,
            || {},
            BindingOptions::default().with_logical_match_policy(LogicalMatchPolicy::Consumed),
        )
        .unwrap();
    assert!(matches!(
        dispatcher.process_event(&observed),
        MatchResult::Matched {
            action: Action::Callback(_),
            ..
        }
    ));
    dispatcher
        .register_pattern(
            BindingPattern::Physical(Hotkey::with_modifiers(Key::DIGIT1, ModifierSet::SHIFT)),
            Action::Suppress,
            BindingOptions::default(),
        )
        .unwrap();
    // One dispatch resolves both identities using the core's physical priority.
    assert!(matches!(
        dispatcher.process_event(&observed),
        MatchResult::Matched {
            action: Action::Suppress,
            ..
        }
    ));
}

#[test]
fn sequence_steps_remain_exact_with_consumed_observations() {
    let mut state = state();
    select(&mut state, SHIFT, 0);
    let observed = event(&state, 2, Key::DIGIT1, ModifierSet::SHIFT);
    let mut dispatcher = Dispatcher::new();
    dispatcher
        .register_sequence_pattern(
            r#"logical:"!", logical:"a""#.parse().unwrap(),
            Action::Suppress,
            kbd::sequence::SequenceOptions::default(),
        )
        .unwrap();
    assert!(matches!(
        dispatcher.process_event(&observed),
        MatchResult::NoMatch
    ));
    dispatcher
        .register_sequence_pattern(
            r#"Shift+logical:"!", logical:"a""#.parse().unwrap(),
            Action::Suppress,
            kbd::sequence::SequenceOptions::default(),
        )
        .unwrap();
    assert!(matches!(
        dispatcher.process_event(&observed),
        MatchResult::Pending {
            steps_matched: 1,
            ..
        }
    ));
    let mut release = observed.clone();
    release.transition = KeyTransition::Release;
    dispatcher.process_event(&release);
    select(&mut state, 0, 0);
    let next = event(&state, 30, Key::A, ModifierSet::NONE);
    assert!(matches!(
        dispatcher.process_event(&next),
        MatchResult::Matched { .. }
    ));
}

#[test]
fn layout_changes_during_a_hold_use_core_physical_lifetime_and_cancellation() {
    let mut state = state();
    let mut dispatcher = Dispatcher::new();
    dispatcher
        .register_tap_hold(
            Key::Q,
            Action::Suppress,
            || {},
            kbd::tap_hold::TapHoldOptions::default()
                .with_threshold(std::time::Duration::from_secs(60)),
        )
        .unwrap();
    let mut observed = event(&state, 16, Key::Q, ModifierSet::NONE);
    assert_eq!(observed.logical, Some(character("q")));
    dispatcher.process_event_from_source(&observed, 7);
    select(&mut state, SHIFT, 1);
    observed.transition = KeyTransition::Release;
    enrich(&mut observed, &state, evdev_keycode(16), MAPPING);
    assert_eq!(observed.logical, Some(character("A")));
    assert!(matches!(
        dispatcher.process_event_from_source(&observed, 7),
        MatchResult::Matched {
            action: Action::Suppress,
            ..
        }
    ));

    observed.transition = KeyTransition::Press;
    dispatcher.process_event_from_source(&observed, 7);
    dispatcher.cancel_source(Some(7));
    observed.transition = KeyTransition::Release;
    assert!(matches!(
        dispatcher.process_event_from_source(&observed, 7),
        MatchResult::Ignored
    ));
}
