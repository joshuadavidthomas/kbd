//! Explicit pattern syntax is independent of the legacy physical grammar.

use kbd::hotkey::Hotkey;
use kbd::hotkey::HotkeySequence;
use kbd::hotkey::ModifierSet;
use kbd::key::Key;
use kbd::observation::BindingPattern;
use kbd::observation::LogicalKeyValue;
use kbd::observation::NamedKey;
use kbd::sequence::BindingSequence;

fn character(text: &str) -> BindingPattern {
    BindingPattern::logical(LogicalKeyValue::Character(text.into()), ModifierSet::NONE)
}

#[test]
fn explicit_patterns_have_independent_semantic_and_canonical_expectations() {
    let cases = [
        (
            "ctrl+return",
            "Ctrl+physical:Enter",
            BindingPattern::Physical(Hotkey::with_modifiers(Key::ENTER, ModifierSet::CTRL)),
        ),
        (
            "Ctrl+physical:A",
            "Ctrl+physical:A",
            BindingPattern::Physical(Hotkey::with_modifiers(Key::A, ModifierSet::CTRL)),
        ),
        (
            "logical:Enter",
            "logical:Enter",
            BindingPattern::logical(NamedKey::Enter, ModifierSet::NONE),
        ),
        (
            r#"logical:"Enter""#,
            r#"logical:"Enter""#,
            character("Enter"),
        ),
        (r#"logical:"a""#, r#"logical:"a""#, character("a")),
        (r#"logical:"A""#, r#"logical:"A""#, character("A")),
        (r#"logical:"+,""#, r#"logical:"+,""#, character("+,")),
        (r#"logical:" a ""#, r#"logical:" a ""#, character(" a ")),
        (r#"logical:"\"\\""#, r#"logical:"\"\\""#, character("\"\\")),
        (r#"logical:"\u00e9""#, r#"logical:"é""#, character("é")),
        (
            "logical:\"e\u{301}\"",
            "logical:\"e\u{301}\"",
            character("e\u{301}"),
        ),
        (r#"logical:"👩‍💻""#, r#"logical:"👩‍💻""#, character("👩‍💻")),
        (r#"logical:"""#, r#"logical:"""#, character("")),
    ];
    for (input, canonical, expected) in cases {
        let parsed: BindingPattern = input.parse().unwrap();
        assert_eq!(parsed, expected, "{input}");
        assert_eq!(parsed.to_string(), canonical, "{input}");
        assert_eq!(canonical.parse::<BindingPattern>().unwrap(), expected);
    }
}

#[test]
fn malformed_patterns_are_not_reinterpreted_as_characters() {
    for input in [
        "",
        "Ctrl++A",
        "A+B",
        r#""a""#,
        "logical:a",
        "logical:NoSuchKey",
        r#"physical:"a""#,
        "text:A",
        r#"logical:"a"junk"#,
        r#"logical:"a"#,
        r#"logical:"\q""#,
        r#"logical:"\uD800""#,
        r#"logical:"\uZZZZ""#,
        "physical:logical:A",
        r#"logical:"a"+physical:B"#,
        "Ctrl+logical:",
    ] {
        assert!(input.parse::<BindingPattern>().is_err(), "{input}");
    }
    for input in [",A", "A,", "A,,B", r#"logical:",", "#] {
        assert!(input.parse::<BindingSequence>().is_err(), "{input}");
    }
}

#[test]
fn mixed_sequence_delimiters_do_not_split_quoted_keys() {
    let sequence: BindingSequence = r#" Ctrl+physical:K , logical:"+," , logical:Enter "#
        .parse()
        .unwrap();
    let expected = vec![
        Hotkey::with_modifiers(Key::K, ModifierSet::CTRL).into(),
        character("+,"),
        BindingPattern::logical(NamedKey::Enter, ModifierSet::NONE),
    ];
    assert_eq!(sequence.steps(), expected);
    assert_eq!(
        sequence.to_string(),
        r#"Ctrl+physical:K, logical:"+,", logical:Enter"#
    );
    assert!(BindingSequence::new(vec![]).is_err());
}

#[test]
fn legacy_physical_parsing_display_and_emission_types_are_unchanged() {
    for (input, canonical) in [
        ("ctrl+a", "Ctrl+A"),
        ("Ctrl", "ControlLeft"),
        ("Ctrl+Comma", "Ctrl+Comma"),
        ("Alt+Ctrl+Shift+A", "Ctrl+Shift+Alt+A"),
    ] {
        let hotkey: Hotkey = input.parse().unwrap();
        assert_eq!(hotkey.to_string(), canonical);
        assert_eq!(canonical.parse::<Hotkey>().unwrap(), hotkey);
    }
    let sequence: HotkeySequence = "Ctrl+K, Ctrl+C".parse().unwrap();
    assert_eq!(sequence.to_string(), "Ctrl+K, Ctrl+C");
    assert_eq!(
        BindingSequence::from(sequence.clone()).steps(),
        &[
            BindingPattern::Physical(Hotkey::with_modifiers(Key::K, ModifierSet::CTRL)),
            BindingPattern::Physical(Hotkey::with_modifiers(Key::C, ModifierSet::CTRL)),
        ]
    );
    for logical in [r#"logical:"a""#, "logical:Enter"] {
        assert!(logical.parse::<Hotkey>().is_err());
        assert!(logical.parse::<HotkeySequence>().is_err());
    }
}

#[cfg(feature = "serde")]
#[test]
fn serde_strings_preserve_domains_and_escaping_from_owned_readers() {
    let cases = [
        (
            BindingPattern::Physical(Hotkey::new(Key::ENTER)),
            "physical:Enter",
        ),
        (
            BindingPattern::logical(NamedKey::Enter, ModifierSet::NONE),
            "logical:Enter",
        ),
        (character("Enter"), r#"logical:"Enter""#),
        (character("+,\"\\\n"), r#"logical:"+,\"\\\n""#),
    ];
    for (pattern, canonical) in cases {
        let json = serde_json::to_string(canonical).unwrap();
        assert_eq!(serde_json::to_string(&pattern).unwrap(), json);
        assert_eq!(
            serde_json::from_str::<BindingPattern>(&json).unwrap(),
            pattern
        );
        assert_eq!(
            serde_json::from_reader::<_, BindingPattern>(json.as_bytes()).unwrap(),
            pattern
        );
    }
    let text = r#"physical:K, logical:",", logical:Enter"#;
    let expected = BindingSequence::new(vec![
        Hotkey::new(Key::K).into(),
        character(","),
        BindingPattern::logical(NamedKey::Enter, ModifierSet::NONE),
    ])
    .unwrap();
    let json = serde_json::to_string(text).unwrap();
    assert_eq!(serde_json::to_string(&expected).unwrap(), json);
    assert_eq!(
        serde_json::from_reader::<_, BindingSequence>(json.as_bytes()).unwrap(),
        expected
    );
    assert_eq!(
        serde_json::to_string(&Hotkey::new(Key::A)).unwrap(),
        r#""A""#
    );
    let physical: HotkeySequence = "Ctrl+K, Ctrl+C".parse().unwrap();
    assert_eq!(
        serde_json::to_string(&physical).unwrap(),
        r#""Ctrl+K, Ctrl+C""#
    );
}

#[test]
fn consumed_modifiers_are_opt_in_and_required_modifiers_stay_required() {
    use kbd::key_state::KeyTransition;
    use kbd::observation::KeyboardObservation;
    use kbd::observation::LogicalModifiers;
    use kbd::observation::ModifierObservation;
    use kbd::observation::ModifierState;
    use kbd::policy::LogicalMatchPolicy::Consumed;
    use kbd::policy::LogicalMatchPolicy::Exact;
    let shift_ctrl = ModifierSet::SHIFT.union(ModifierSet::CTRL);
    // Independently specified expected results: required, active, consumed,
    // exact match, consumption-aware match.
    let cases = [
        (
            ModifierSet::NONE,
            ModifierSet::NONE,
            Some(ModifierSet::NONE),
            true,
            true,
        ),
        (
            ModifierSet::NONE,
            ModifierSet::SHIFT,
            Some(ModifierSet::SHIFT),
            false,
            true,
        ),
        (
            ModifierSet::SHIFT,
            ModifierSet::SHIFT,
            Some(ModifierSet::SHIFT),
            true,
            true,
        ),
        (
            ModifierSet::SHIFT,
            ModifierSet::NONE,
            Some(ModifierSet::SHIFT),
            false,
            false,
        ),
        (
            ModifierSet::CTRL,
            shift_ctrl,
            Some(ModifierSet::SHIFT),
            false,
            true,
        ),
        (
            ModifierSet::SHIFT,
            shift_ctrl,
            Some(ModifierSet::SHIFT),
            false,
            false,
        ),
        (
            shift_ctrl,
            ModifierSet::SHIFT,
            Some(shift_ctrl),
            false,
            false,
        ),
        (
            ModifierSet::NONE,
            shift_ctrl,
            Some(ModifierSet::SHIFT),
            false,
            false,
        ),
        (ModifierSet::NONE, shift_ctrl, Some(shift_ctrl), false, true),
        (ModifierSet::NONE, ModifierSet::SHIFT, None, false, false),
        (ModifierSet::SHIFT, ModifierSet::SHIFT, None, true, true),
    ];
    for (required, active, consumed, exact, aware) in cases {
        let mut event =
            KeyboardObservation::from_hotkey(Hotkey::new(Key::DIGIT1), KeyTransition::Press);
        event.logical = Some(LogicalKeyValue::Character("!".into()).into());
        event.modifier_observation = Some(ModifierObservation {
            physical: ModifierState::new(ModifierSet::SHIFT, ModifierSet::STANDARD),
            logical: Some(LogicalModifiers {
                state: ModifierState::new(active, ModifierSet::STANDARD),
                consumed,
            }),
        });
        let pattern = BindingPattern::logical(LogicalKeyValue::Character("!".into()), required);
        assert_eq!(
            pattern.matches_with_policy(&event, Exact),
            exact,
            "{required:?} {active:?} {consumed:?}"
        );
        assert_eq!(
            pattern.matches_with_policy(&event, Consumed),
            aware,
            "{required:?} {active:?} {consumed:?}"
        );
        assert!(
            BindingPattern::Physical(Hotkey::with_modifiers(Key::DIGIT1, ModifierSet::SHIFT))
                .matches(&event)
        );
    }
}

#[test]
fn enriched_logical_matching_dispatches_once_and_works_in_layers() {
    use kbd::action::Action;
    use kbd::binding::BindingOptions;
    use kbd::dispatcher::Dispatcher;
    use kbd::dispatcher::MatchResult;
    use kbd::key_state::KeyTransition;
    use kbd::layer::Layer;
    use kbd::observation::KeyboardObservation;
    use kbd::observation::LogicalModifiers;
    use kbd::observation::ModifierObservation;
    use kbd::observation::ModifierState;
    use kbd::policy::LogicalMatchPolicy;
    let pattern = BindingPattern::logical(NamedKey::Tab, ModifierSet::NONE);
    let options = BindingOptions::default().with_logical_match_policy(LogicalMatchPolicy::Consumed);
    let mut event = KeyboardObservation::from_hotkey(
        Hotkey::with_modifiers(Key::TAB, ModifierSet::SHIFT),
        KeyTransition::Press,
    );
    event.logical = Some(NamedKey::Tab.into());
    event.modifier_observation = Some(ModifierObservation {
        physical: ModifierState::new(ModifierSet::SHIFT, ModifierSet::STANDARD),
        logical: Some(LogicalModifiers {
            state: ModifierState::new(ModifierSet::SHIFT, ModifierSet::STANDARD),
            consumed: Some(ModifierSet::SHIFT),
        }),
    });
    let mut d = Dispatcher::new();
    d.register_pattern(
        pattern.clone(),
        Action::EmitHotkey(Key::L.into()),
        options.clone(),
    )
    .unwrap();
    assert!(
        matches!(d.process_event(&event), MatchResult::Matched { action: Action::EmitHotkey(h), .. } if h.key() == Key::L)
    );
    d.register(
        Hotkey::with_modifiers(Key::TAB, ModifierSet::SHIFT),
        Action::EmitHotkey(Key::P.into()),
    )
    .unwrap();
    assert!(
        matches!(d.process_event(&event), MatchResult::Matched { action: Action::EmitHotkey(h), .. } if h.key() == Key::P)
    );
    let layer =
        Layer::new("logical").bind_pattern(pattern, Action::EmitHotkey(Key::Z.into()), options);
    d.define_layer(layer).unwrap();
    d.push_layer("logical").unwrap();
    assert!(
        matches!(d.process_event(&event), MatchResult::Matched { action: Action::EmitHotkey(h), .. } if h.key() == Key::Z)
    );
}

#[test]
fn extended_modifier_knowledge_and_physical_associations_are_honest() {
    use kbd::hotkey::Modifier;
    use kbd::key_state::KeyTransition;
    use kbd::observation::KeyboardObservation;
    use kbd::observation::ModifierObservation;
    use kbd::observation::ModifierState;
    assert_eq!(Modifier::Fn.keys(), Some([Key::FN].as_slice()));
    assert!(Modifier::AltGraph.keys().is_none());
    assert!(Key::try_from(Modifier::AltGraph).is_err());
    assert_eq!(Modifier::from_key(Key::ALT_RIGHT), Some(Modifier::Alt)); // physical only
    for modifier in [Modifier::Fn, Modifier::AltGraph] {
        let pattern: BindingPattern = format!("{modifier}+physical:A").parse().unwrap();
        assert_eq!(pattern.to_string(), format!("{modifier}+physical:A"));
        for (active, known, matches) in [
            (ModifierSet::NONE, ModifierSet::STANDARD, false),
            (ModifierSet::NONE, ModifierSet::ALL_MODIFIERS, false),
            (
                ModifierSet::from(modifier),
                ModifierSet::ALL_MODIFIERS,
                true,
            ),
        ] {
            let mut event = KeyboardObservation::from_hotkey(Key::A.into(), KeyTransition::Press);
            event.modifier_observation = Some(ModifierObservation {
                physical: ModifierState::new(active, known),
                logical: None,
            });
            assert_eq!(pattern.matches(&event), matches);
        }
    }
}

#[test]
fn overlapping_logical_patterns_use_registration_order_not_pattern_or_id_order() {
    use kbd::action::Action;
    use kbd::binding::Binding;
    use kbd::binding::BindingId;
    use kbd::binding::BindingOptions;
    use kbd::device::DeviceContext;
    use kbd::device::DeviceFilter;
    use kbd::device::DeviceInfo;
    use kbd::dispatcher::Dispatcher;
    use kbd::dispatcher::MatchResult;
    use kbd::key_state::KeyTransition;
    use kbd::observation::KeyboardObservation;
    use kbd::observation::LogicalModifiers;
    use kbd::observation::ModifierObservation;
    use kbd::observation::ModifierState;
    use kbd::policy::LogicalMatchPolicy;

    let info = DeviceInfo::new("test keyboard", 1, 2);
    let device = DeviceContext::new(10, &info);
    let mut event = KeyboardObservation::from_hotkey(
        Hotkey::with_modifiers(Key::DIGIT1, ModifierSet::SHIFT),
        KeyTransition::Press,
    );
    event.logical = Some(LogicalKeyValue::Character("!".into()).into());
    event.modifier_observation = Some(ModifierObservation {
        physical: ModifierState::new(ModifierSet::SHIFT, ModifierSet::STANDARD),
        logical: Some(LogicalModifiers {
            state: ModifierState::new(ModifierSet::SHIFT, ModifierSet::STANDARD),
            consumed: Some(ModifierSet::SHIFT),
        }),
    });
    for device_scoped in [false, true] {
        for shift_first in [false, true] {
            for reverse_id_creation in [false, true] {
                let mut ids = [BindingId::new(), BindingId::new()];
                if reverse_id_creation {
                    ids.reverse();
                }
                let requirements = if shift_first {
                    [ModifierSet::SHIFT, ModifierSet::NONE]
                } else {
                    [ModifierSet::NONE, ModifierSet::SHIFT]
                };
                let mut options = BindingOptions::default()
                    .with_logical_match_policy(LogicalMatchPolicy::Consumed);
                if device_scoped {
                    options = options.with_device(DeviceFilter::name_contains("keyboard"));
                }
                let binding = |index: usize, action: Key| {
                    Binding::new(
                        ids[index],
                        BindingPattern::logical(
                            LogicalKeyValue::Character("!".into()),
                            requirements[index],
                        ),
                        Action::EmitHotkey(action.into()),
                    )
                    .with_options(options.clone())
                };
                let mut dispatcher = Dispatcher::new();
                dispatcher.register_binding(binding(0, Key::A)).unwrap();
                dispatcher.register_binding(binding(1, Key::B)).unwrap();
                let result = dispatcher.process_event_with_device(&event, &device);
                assert!(
                    matches!(result, MatchResult::Matched { action: Action::EmitHotkey(h), .. } if h.key() == Key::B),
                    "device={device_scoped}, shift_first={shift_first}, reverse_ids={reverse_id_creation}"
                );
                // Re-registering an old ID creates a new registration position.
                dispatcher.unregister(ids[0]);
                dispatcher.register_binding(binding(0, Key::C)).unwrap();
                assert!(
                    matches!(dispatcher.process_event_with_device(&event, &device),
                    MatchResult::Matched { action: Action::EmitHotkey(h), .. } if h.key() == Key::C)
                );
            }
        }
    }
}

#[test]
fn primary_is_resolved_explicitly_before_registration() {
    use kbd::action::Action;
    use kbd::binding::BindingOptions;
    use kbd::dispatcher::Dispatcher;
    use kbd::observation::ConfiguredPattern;
    use kbd::policy::PrimaryModifier;
    let config: ConfiguredPattern = "Primary+Shift+logical:\"+\"".parse().unwrap();
    assert_eq!(config.to_string(), "Primary+Shift+logical:\"+\"");
    assert_eq!(
        config.resolve(PrimaryModifier::Ctrl).to_string(),
        "Ctrl+Shift+logical:\"+\""
    );
    assert_eq!(
        config.resolve(PrimaryModifier::Super).to_string(),
        "Shift+Super+logical:\"+\""
    );
    assert!("Primary".parse::<kbd::hotkey::Modifier>().is_err());
    assert!("Primary+A".parse::<Hotkey>().is_err());
    let mut d = Dispatcher::new();
    d.register_configured_pattern(
        &config,
        PrimaryModifier::Ctrl,
        Action::Suppress,
        BindingOptions::default(),
    )
    .unwrap();
    assert!(
        d.register_pattern(
            config.resolve(PrimaryModifier::Ctrl),
            Action::Suppress,
            BindingOptions::default()
        )
        .is_err()
    );
    #[cfg(feature = "serde")]
    assert_eq!(
        serde_json::from_str::<ConfiguredPattern>(&serde_json::to_string(&config).unwrap())
            .unwrap(),
        config
    );
}
