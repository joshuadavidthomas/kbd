#![allow(missing_docs)]
mod utils;

use kbd::action::Action;
use kbd::key::Key;
use kbd::layer::Layer;
use kbd::layer::LayerName;
use kbd_global::error::Error;

#[test]
fn push_layer_succeeds_for_defined_layer() {
    let manager = utils::test_manager();

    let layer = Layer::new("nav").bind(Key::H, Action::Suppress).unwrap();
    manager.define_layer(layer).unwrap();

    let result = manager.push_layer("nav");
    assert!(result.is_ok());
}

#[test]
fn push_undefined_layer_returns_error() {
    let manager = utils::test_manager();

    let result = manager.push_layer("nonexistent");
    assert!(matches!(result, Err(Error::LayerNotDefined)));
}

#[test]
fn pop_layer_returns_popped_name() {
    let manager = utils::test_manager();

    let layer = Layer::new("nav").bind(Key::H, Action::Suppress).unwrap();
    manager.define_layer(layer).unwrap();
    manager.push_layer("nav").unwrap();

    let popped = manager.pop_layer().unwrap();
    assert_eq!(popped.as_str(), "nav");
}

#[test]
fn pop_empty_stack_returns_error() {
    let manager = utils::test_manager();

    let result = manager.pop_layer();
    assert!(matches!(result, Err(Error::EmptyLayerStack)));
}

#[test]
fn toggle_layer_on_and_off() {
    let manager = utils::test_manager();

    let layer = Layer::new("nav").bind(Key::H, Action::Suppress).unwrap();
    manager.define_layer(layer).unwrap();

    // Toggle on
    manager.toggle_layer("nav").unwrap();

    // Toggle off (should succeed — layer was active)
    manager.toggle_layer("nav").unwrap();

    // Pop should fail — stack is empty after toggle off
    let result = manager.pop_layer();
    assert!(matches!(result, Err(Error::EmptyLayerStack)));
}

#[test]
fn toggle_undefined_layer_returns_error() {
    let manager = utils::test_manager();

    let result = manager.toggle_layer("nonexistent");
    assert!(matches!(result, Err(Error::LayerNotDefined)));
}

#[test]
fn push_same_layer_twice_is_allowed() {
    let manager = utils::test_manager();

    let layer = Layer::new("nav").bind(Key::H, Action::Suppress).unwrap();
    manager.define_layer(layer).unwrap();

    // Pushing the same layer twice should work (stacking)
    manager.push_layer("nav").unwrap();
    manager.push_layer("nav").unwrap();

    // Pop twice should both succeed
    assert_eq!(manager.pop_layer().unwrap().as_str(), "nav");
    assert_eq!(manager.pop_layer().unwrap().as_str(), "nav");
}

#[test]
fn push_pop_multiple_layers_in_order() {
    let manager = utils::test_manager();

    let nav = Layer::new("nav").bind(Key::H, Action::Suppress).unwrap();
    let edit = Layer::new("edit").bind(Key::I, Action::Suppress).unwrap();
    manager.define_layer(nav).unwrap();
    manager.define_layer(edit).unwrap();

    manager.push_layer("nav").unwrap();
    manager.push_layer("edit").unwrap();

    // LIFO order
    assert_eq!(manager.pop_layer().unwrap().as_str(), "edit");
    assert_eq!(manager.pop_layer().unwrap().as_str(), "nav");
}

#[test]
fn layer_name_accepts_string_and_str() {
    let manager = utils::test_manager();

    let layer = Layer::new("nav").bind(Key::H, Action::Suppress).unwrap();
    manager.define_layer(layer).unwrap();

    // &str
    manager.push_layer("nav").unwrap();
    manager.pop_layer().unwrap();

    // String
    manager.push_layer(String::from("nav")).unwrap();
    manager.pop_layer().unwrap();

    // LayerName
    manager.push_layer(LayerName::new("nav")).unwrap();
    manager.pop_layer().unwrap();
}
