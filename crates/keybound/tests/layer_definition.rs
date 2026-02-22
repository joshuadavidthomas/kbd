#![cfg(feature = "evdev")]

use std::time::Duration;

use keybound::Action;
use keybound::Error;
use keybound::Hotkey;
use keybound::HotkeyManager;
use keybound::Key;
use keybound::Layer;
use keybound::LayerOptions;
use keybound::Modifier;
use keybound::UnmatchedKeyBehavior;

#[test]
fn define_layer_via_manager() {
    let manager = HotkeyManager::new().expect("manager should initialize");

    let layer = Layer::new("nav")
        .bind(Key::H, Action::Swallow)
        .bind(Key::J, Action::Swallow)
        .bind(Key::K, Action::Swallow)
        .bind(Key::L, Action::Swallow);

    let result = manager.define_layer(layer);
    assert!(result.is_ok());
}

#[test]
fn define_duplicate_layer_returns_error() {
    let manager = HotkeyManager::new().expect("manager should initialize");

    let layer1 = Layer::new("nav").bind(Key::H, Action::Swallow);
    manager.define_layer(layer1).expect("first should succeed");

    let layer2 = Layer::new("nav").bind(Key::J, Action::Swallow);
    let result = manager.define_layer(layer2);
    assert!(matches!(result, Err(Error::LayerAlreadyDefined)));
}

#[test]
fn define_layers_with_different_names_succeeds() {
    let manager = HotkeyManager::new().expect("manager should initialize");

    let nav = Layer::new("nav").bind(Key::H, Action::Swallow);
    let edit = Layer::new("edit").bind(Key::I, Action::Swallow);

    manager.define_layer(nav).expect("nav should succeed");
    manager.define_layer(edit).expect("edit should succeed");
}

#[test]
fn define_empty_layer_succeeds() {
    let manager = HotkeyManager::new().expect("manager should initialize");

    let layer = Layer::new("empty");
    let result = manager.define_layer(layer);
    assert!(result.is_ok());
}

#[test]
fn define_layer_with_all_options() {
    let manager = HotkeyManager::new().expect("manager should initialize");

    let layer = Layer::new("oneshot-nav")
        .bind(Key::H, Action::Swallow)
        .swallow()
        .oneshot(1)
        .timeout(Duration::from_secs(5));

    let result = manager.define_layer(layer);
    assert!(result.is_ok());
}

#[test]
fn layer_builder_produces_correct_state() {
    let layer = Layer::new("test")
        .bind(
            Hotkey::new(Key::A).modifier(Modifier::Ctrl),
            Action::Swallow,
        )
        .bind(Key::B, || println!("fired"))
        .swallow()
        .oneshot(2)
        .timeout(Duration::from_millis(500));

    assert_eq!(layer.name().as_str(), "test");
    assert_eq!(layer.binding_count(), 2);
    assert_eq!(layer.options().unmatched, UnmatchedKeyBehavior::Swallow);
    assert_eq!(layer.options().oneshot, Some(2));
    assert_eq!(layer.options().timeout, Some(Duration::from_millis(500)));
}

#[test]
fn layer_default_options() {
    let options = LayerOptions::default();
    assert_eq!(options.oneshot, None);
    assert_eq!(options.unmatched, UnmatchedKeyBehavior::Fallthrough);
    assert_eq!(options.timeout, None);
    assert_eq!(options.description, None);
}

// Phase 3.4: Binding metadata on layers

#[test]
fn layer_options_description_defaults_to_none() {
    let options = LayerOptions::default();
    assert_eq!(options.description, None);
}

#[test]
fn layer_description_sets_label() {
    let layer = Layer::new("nav").description("Navigation keys");
    assert_eq!(
        layer.options().description.as_deref(),
        Some("Navigation keys")
    );
}

#[test]
fn layer_description_chains_with_other_options() {
    let layer = Layer::new("nav")
        .bind(Key::H, Action::Swallow)
        .description("Navigation keys")
        .swallow()
        .oneshot(1)
        .timeout(Duration::from_secs(5));

    assert_eq!(
        layer.options().description.as_deref(),
        Some("Navigation keys")
    );
    assert_eq!(layer.options().unmatched, UnmatchedKeyBehavior::Swallow);
    assert_eq!(layer.options().oneshot, Some(1));
    assert_eq!(layer.binding_count(), 1);
}

#[test]
fn layer_description_preserved_through_define_layer() {
    let manager = HotkeyManager::new().expect("manager should initialize");

    let layer = Layer::new("nav")
        .bind(Key::H, Action::Swallow)
        .description("Navigation keys");

    // If define_layer succeeds, the metadata was accepted by the engine.
    // Full introspection comes in Phase 3.5.
    let result = manager.define_layer(layer);
    assert!(result.is_ok());
}
