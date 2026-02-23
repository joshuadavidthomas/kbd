//! Tests that `keybound` re-exports all kbd-core public types
//! and that the public API surface is correct after the facade rewire.

// All these imports should work through `keybound::` — they come from kbd-core.
use keybound::Action;
use keybound::ActiveLayerInfo;
use keybound::BindingId;
use keybound::BindingInfo;
use keybound::BindingLocation;
use keybound::BindingOptions;
use keybound::ConflictInfo;
use keybound::DeviceFilter;
use keybound::Error;
use keybound::Hotkey;
use keybound::HotkeySequence;
use keybound::Key;
use keybound::KeyTransition;
use keybound::Layer;
use keybound::LayerName;
use keybound::LayerOptions;
use keybound::MatchResult;
use keybound::Matcher;
use keybound::Modifier;
use keybound::OverlayVisibility;
use keybound::ParseHotkeyError;
use keybound::Passthrough;
use keybound::RegisteredBinding;
use keybound::ShadowedStatus;
use keybound::UnmatchedKeyBehavior;

#[test]
fn core_types_reexported_through_keybound() {
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
fn matcher_reexported_through_keybound() {
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
