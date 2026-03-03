#![allow(missing_docs)]
//! Tests that `kbd-global` re-exports all kbd public types
//! and that the public API surface is correct after the runtime rewire.

// All these imports should work through `kbd_global::` — they come from kbd.
use kbd_global::Action;
use kbd_global::ActiveLayerInfo;
use kbd_global::BindingId;
use kbd_global::BindingInfo;
use kbd_global::BindingLocation;
use kbd_global::BindingOptions;
use kbd_global::ConflictInfo;
use kbd_global::DeviceFilter;
use kbd_global::Error;
use kbd_global::Hotkey;
use kbd_global::HotkeySequence;
use kbd_global::Key;
use kbd_global::KeyTransition;
use kbd_global::Layer;
use kbd_global::LayerName;
use kbd_global::LayerOptions;
use kbd_global::MatchResult;
use kbd_global::Matcher;
use kbd_global::Modifier;
use kbd_global::OverlayVisibility;
use kbd_global::ParseHotkeyError;
use kbd_global::Passthrough;
use kbd_global::RegisteredBinding;
use kbd_global::ShadowedStatus;
use kbd_global::UnmatchedKeyBehavior;

#[test]
fn core_types_reexported_through_kbd_global() {
    // Key types
    let hotkey: Hotkey = "Ctrl+C".parse().unwrap();
    assert_eq!(hotkey.key(), Key::C);
    assert_eq!(hotkey.modifiers(), &[Modifier::Ctrl]);

    // Sequence
    let seq: HotkeySequence = "Ctrl+K, Ctrl+C".parse().unwrap();
    assert_eq!(seq.steps().len(), 2);

    // Action
    let _action: Action = Action::from(|| {});

    // Binding
    let id = BindingId::new();
    let _binding = RegisteredBinding::new(id, hotkey, Action::Swallow);

    // Options
    let opts = BindingOptions::default()
        .with_passthrough(Passthrough::Enabled)
        .with_overlay_visibility(OverlayVisibility::Hidden)
        .with_description("test");
    assert_eq!(opts.passthrough(), Passthrough::Enabled);

    // Layer
    let layer = Layer::new("test").bind(Key::H, Action::Swallow).swallow();
    assert_eq!(layer.name().as_str(), "test");
    assert_eq!(layer.options().unmatched(), UnmatchedKeyBehavior::Swallow);

    // LayerName
    let name = LayerName::from("test");
    assert_eq!(name.as_str(), "test");

    // LayerOptions
    let _opts = LayerOptions::default();

    // Parse error
    let err = "not+a+valid+key+combo+++".parse::<Hotkey>();
    assert!(err.is_err());
    let _parse_err: ParseHotkeyError = err.unwrap_err();
}

#[test]
fn matcher_reexported_through_kbd_global() {
    let mut matcher = Matcher::new();
    let hotkey = Hotkey::new(Key::C).modifier(Modifier::Ctrl);
    matcher
        .register(hotkey.clone(), Action::Swallow)
        .expect("register should succeed");

    let result = matcher.process(&hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::Matched { .. }));
}

#[test]
fn introspection_types_reexported() {
    // Verify the types exist and have expected shapes via function signatures
    let _: fn() -> Vec<BindingInfo> = || vec![];
    let _: fn() -> Vec<ActiveLayerInfo> = || vec![];
    let _: fn() -> Vec<ConflictInfo> = || vec![];

    // Verify enum variants are accessible
    assert_eq!(BindingLocation::Global, BindingLocation::Global);
    assert_eq!(ShadowedStatus::Active, ShadowedStatus::Active);
}

#[test]
fn device_filter_reexported() {
    let _filter = DeviceFilter::NamePattern("keyboard".into());
}

#[test]
fn error_variants_accessible() {
    let err = Error::AlreadyRegistered;
    let msg = format!("{err}");
    assert!(!msg.is_empty());

    let err = Error::LayerNotDefined;
    let msg = format!("{err}");
    assert!(!msg.is_empty());
}
