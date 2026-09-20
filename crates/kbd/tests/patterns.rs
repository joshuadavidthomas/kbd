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
