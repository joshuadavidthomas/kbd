#![allow(missing_docs)]
mod utils;

use std::time::Duration;

use kbd::action::Action;
use kbd::hotkey::Hotkey;
use kbd::hotkey::Modifier;
use kbd::key::Key;
use kbd::layer::Layer;
use kbd::layer::LayerOptions;
use kbd::layer::UnmatchedKeys;
use kbd_global::error::LayerError;

#[test]
fn define_layer_via_manager() {
    let manager = utils::test_manager();

    let layer = Layer::new("nav")
        .bind(Key::H, Action::Suppress)
        .unwrap()
        .bind(Key::J, Action::Suppress)
        .unwrap()
        .bind(Key::K, Action::Suppress)
        .unwrap()
        .bind(Key::L, Action::Suppress)
        .unwrap();

    let result = manager.define_layer(layer);
    assert!(result.is_ok());
}

#[test]
fn define_duplicate_layer_returns_error() {
    let manager = utils::test_manager();

    let layer1 = Layer::new("nav").bind(Key::H, Action::Suppress).unwrap();
    manager.define_layer(layer1).expect("first should succeed");

    let layer2 = Layer::new("nav").bind(Key::J, Action::Suppress).unwrap();
    let result = manager.define_layer(layer2);
    assert!(matches!(result, Err(LayerError::AlreadyDefined)));
}

#[test]
fn define_layers_with_different_names_succeeds() {
    let manager = utils::test_manager();

    let nav = Layer::new("nav").bind(Key::H, Action::Suppress).unwrap();
    let edit = Layer::new("edit").bind(Key::I, Action::Suppress).unwrap();

    manager.define_layer(nav).expect("nav should succeed");
    manager.define_layer(edit).expect("edit should succeed");
}

#[test]
fn define_empty_layer_succeeds() {
    let manager = utils::test_manager();

    let layer = Layer::new("empty");
    let result = manager.define_layer(layer);
    assert!(result.is_ok());
}

#[test]
fn define_layer_with_all_options() {
    let manager = utils::test_manager();

    let layer = Layer::new("oneshot-nav")
        .bind(Key::H, Action::Suppress)
        .unwrap()
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
            Action::Suppress,
        )
        .unwrap()
        .bind(Key::B, || println!("fired"))
        .unwrap()
        .swallow()
        .oneshot(2)
        .timeout(Duration::from_millis(500));

    assert_eq!(layer.name().as_str(), "test");
    assert_eq!(layer.binding_count(), 2);
    assert_eq!(layer.options().unmatched(), UnmatchedKeys::Swallow);
    assert_eq!(layer.options().oneshot(), Some(2));
    assert_eq!(layer.options().timeout(), Some(Duration::from_millis(500)));
}

#[test]
fn layer_default_options() {
    let options = LayerOptions::default();
    assert_eq!(options.oneshot(), None);
    assert_eq!(options.unmatched(), UnmatchedKeys::Fallthrough);
    assert_eq!(options.timeout(), None);
    assert_eq!(options.description(), None);
}

// Binding metadata on layers

#[test]
fn layer_options_description_defaults_to_none() {
    let options = LayerOptions::default();
    assert_eq!(options.description(), None);
}

#[test]
fn layer_description_sets_label() {
    let layer = Layer::new("nav").description("Navigation keys");
    assert_eq!(layer.options().description(), Some("Navigation keys"));
}

#[test]
fn layer_description_chains_with_other_options() {
    let layer = Layer::new("nav")
        .bind(Key::H, Action::Suppress)
        .unwrap()
        .description("Navigation keys")
        .swallow()
        .oneshot(1)
        .timeout(Duration::from_secs(5));

    assert_eq!(layer.options().description(), Some("Navigation keys"));
    assert_eq!(layer.options().unmatched(), UnmatchedKeys::Swallow);
    assert_eq!(layer.options().oneshot(), Some(1));
    assert_eq!(layer.binding_count(), 1);
}

#[test]
fn layer_description_preserved_through_define_layer() {
    let manager = utils::test_manager();

    let layer = Layer::new("nav")
        .bind(Key::H, Action::Suppress)
        .unwrap()
        .description("Navigation keys");

    // If define_layer succeeds, the metadata was accepted by the engine.
    // Full introspection tested separately.
    let result = manager.define_layer(layer);
    assert!(result.is_ok());
}
