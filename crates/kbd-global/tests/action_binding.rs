#![allow(missing_docs)]
mod common;

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use kbd::action::Action;
use kbd::binding::BindingId;
use kbd::binding::BindingOptions;
use kbd::binding::KeyPropagation;
use kbd::binding::OverlayVisibility;
use kbd::hotkey::Hotkey;
use kbd::hotkey::Modifier;
use kbd::key::Key;

#[test]
fn action_from_closure_runs_callback() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let call_count_for_closure = Arc::clone(&call_count);

    let action = Action::from(move || {
        call_count_for_closure.fetch_add(1, Ordering::SeqCst);
    });

    match action {
        Action::Callback(callback) => {
            callback();
        }
        _ => panic!("expected callback action"),
    }

    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

#[test]
fn generated_binding_ids_are_unique() {
    let mut ids = HashSet::new();

    for _ in 0..128 {
        let id = BindingId::new();
        assert!(ids.insert(id));
    }
}

#[test]
fn binding_options_default_to_consuming_events() {
    let options = BindingOptions::default();
    assert_eq!(options.propagation(), KeyPropagation::Stop);
}

#[test]
fn action_variants_exist_for_future_features() {
    let _ = Action::EmitHotkey(Hotkey::new(Key::ESCAPE).modifier(Modifier::Ctrl));
    let _ = Action::PushLayer("nav".into());
    let _ = Action::ToggleLayer("nav".into());
    let _ = Action::PopLayer;
    let _ = Action::Suppress;
}

// Binding metadata

#[test]
fn binding_options_description_defaults_to_none() {
    let options = BindingOptions::default();
    assert_eq!(options.description(), None);
}

#[test]
fn binding_options_with_description_sets_label() {
    let options = BindingOptions::default().with_description("Copy to clipboard");
    assert_eq!(options.description(), Some("Copy to clipboard"));
}

#[test]
fn binding_options_overlay_visibility_defaults_to_visible() {
    let options = BindingOptions::default();
    assert_eq!(options.overlay_visibility(), OverlayVisibility::Visible);
}

#[test]
fn binding_options_with_overlay_visibility_hidden() {
    let options = BindingOptions::default().with_overlay_visibility(OverlayVisibility::Hidden);
    assert_eq!(options.overlay_visibility(), OverlayVisibility::Hidden);
}

#[test]
fn binding_options_chains_all_metadata() {
    let options = BindingOptions::default()
        .with_description("Quit application")
        .with_overlay_visibility(OverlayVisibility::Hidden)
        .with_propagation(KeyPropagation::Continue);

    assert_eq!(options.description(), Some("Quit application"));
    assert_eq!(options.overlay_visibility(), OverlayVisibility::Hidden);
    assert_eq!(options.propagation(), KeyPropagation::Continue);
}

#[test]
fn register_with_options_accepts_metadata() {
    let manager = common::test_manager();

    let options = BindingOptions::default()
        .with_description("Copy to clipboard")
        .with_overlay_visibility(OverlayVisibility::Visible);

    let handle = manager.register_with_options(Key::C, || println!("copy"), options);
    assert!(handle.is_ok());
}

#[test]
fn register_with_options_hidden_binding() {
    let manager = common::test_manager();

    let options = BindingOptions::default()
        .with_description("Internal binding")
        .with_overlay_visibility(OverlayVisibility::Hidden);

    let handle = manager.register_with_options(Key::F12, || {}, options);
    assert!(handle.is_ok());
}
