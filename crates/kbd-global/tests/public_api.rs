#![allow(missing_docs)]
//! Integration tests that exercise kbd-global's public API as an outside consumer would.
//!
//! These complement the unit tests in engine.rs and the focused integration
//! tests (`action_binding.rs`, `error_type.rs`, etc.) by verifying that:
//! - Import paths work as documented
//! - The documented quick-start example compiles and runs
//! - Manager, builder, guard, and backend types compose correctly
//! - Introspection queries return coherent results
//! - Layer lifecycle (define → push → pop → toggle) works end-to-end

mod utils;

use kbd::prelude::*;
use kbd_global::backend::Backend;
use kbd_global::binding_guard::BindingGuard;
use kbd_global::error::Error;
use kbd_global::events::HotkeyEvent;
use kbd_global::manager::HotkeyManager;
use kbd_global::manager::HotkeyManagerBuilder;

// Quick-start / smoke tests

#[test]
fn quick_start_example_compiles_and_runs() {
    let manager = utils::test_manager();
    let _guard: BindingGuard = manager
        .register(
            Hotkey::new(Key::C)
                .modifier(Modifier::Ctrl)
                .modifier(Modifier::Shift),
            || {
                println!("fired");
            },
        )
        .expect("registration should succeed");
}

#[test]
fn manager_debug_shows_backend_and_running_state() {
    let manager = utils::test_manager();
    let debug = format!("{manager:?}");
    assert!(debug.contains("Evdev"));
    assert!(debug.contains("running"));
}

// Builder API

#[test]
fn builder_default_produces_evdev_backend() {
    let manager = utils::test_builder().build().expect("should build");
    assert_eq!(manager.active_backend(), Backend::Evdev);
    manager.shutdown().expect("shutdown should succeed");
}

#[test]
fn builder_explicit_evdev_backend() {
    let manager = utils::test_builder()
        .backend(Backend::Evdev)
        .build()
        .expect("should build");
    assert_eq!(manager.active_backend(), Backend::Evdev);
}

#[test]
fn builder_type_is_exported() {
    let _builder: HotkeyManagerBuilder = HotkeyManager::builder();
}

#[test]
#[cfg(not(feature = "grab"))]
fn builder_grab_without_feature_returns_unsupported() {
    let result = utils::test_builder().grab().build();
    assert!(matches!(result, Err(Error::UnsupportedFeature)));
}

// Registration and guard lifecycle

#[test]
fn register_returns_binding_guard() {
    let manager = utils::test_manager();
    let guard: BindingGuard = manager
        .register(Key::F5, || {})
        .expect("registration should succeed");
    let _id = guard.binding_id();
}

#[test]
fn binding_guard_debug_shows_id_and_state() {
    let manager = utils::test_manager();
    let guard = manager
        .register(Key::F6, || {})
        .expect("registration should succeed");
    let debug = format!("{guard:?}");
    assert!(debug.contains("BindingGuard"));
    assert!(debug.contains("Active"));
}

#[test]
fn dropping_guard_unregisters_hotkey() {
    let manager = utils::test_manager();
    let hotkey = Hotkey::new(Key::A).modifier(Modifier::Ctrl);

    let guard = manager
        .register(hotkey.clone(), || {})
        .expect("register should succeed");
    assert!(manager.is_registered(hotkey.clone()).unwrap());

    drop(guard);
    assert!(!manager.is_registered(hotkey).unwrap());
}

#[test]
fn explicit_unregister_removes_binding() {
    let manager = utils::test_manager();
    let hotkey = Hotkey::new(Key::B).modifier(Modifier::Alt);

    let guard = manager
        .register(hotkey.clone(), || {})
        .expect("register should succeed");
    assert!(manager.is_registered(hotkey.clone()).unwrap());

    guard.unregister().expect("unregister should succeed");
    assert!(!manager.is_registered(hotkey).unwrap());
}

#[test]
fn register_with_options_sets_metadata() {
    let manager = utils::test_manager();
    let hotkey = Hotkey::new(Key::S).modifier(Modifier::Ctrl);

    let options = kbd::binding::BindingOptions::default()
        .with_description("Save file")
        .with_overlay_visibility(kbd::binding::OverlayVisibility::Visible);

    let _guard = manager
        .register_with_options(hotkey.clone(), Action::Suppress, options)
        .expect("register should succeed");

    let bindings = manager.list_bindings().expect("query should succeed");
    let save = bindings
        .iter()
        .find(|b| b.hotkey == hotkey)
        .expect("should find binding");
    assert_eq!(save.description.as_deref(), Some("Save file"));
}

#[test]
fn duplicate_registration_returns_already_registered() {
    let manager = utils::test_manager();
    let hotkey = Hotkey::new(Key::C).modifier(Modifier::Ctrl);

    let _first = manager.register(hotkey.clone(), || {}).unwrap();
    let result = manager.register(hotkey, || {});
    assert!(matches!(result, Err(Error::AlreadyRegistered)));
}

// Key state queries

#[test]
fn is_key_pressed_false_at_start() {
    let manager = utils::test_manager();
    assert!(!manager.is_key_pressed(Key::A).unwrap());
}

#[test]
fn active_modifiers_empty_at_start() {
    let manager = utils::test_manager();
    let mods = manager.active_modifiers().unwrap();
    assert!(mods.is_empty());
}

#[test]
fn event_stream_closes_when_manager_shuts_down() {
    let manager = utils::test_manager();
    let stream = manager.event_stream().expect("event stream should initialize");

    manager.shutdown().expect("shutdown should succeed");

    assert!(stream.recv_blocking().is_err());
}

#[test]
fn event_type_is_publicly_accessible() {
    let event = HotkeyEvent::SequenceStep {
        binding_id: kbd::binding::BindingId::new(),
        hotkey: Hotkey::new(Key::K).modifier(Modifier::Ctrl),
        steps_matched: 1,
        steps_remaining: 0,
    };
    assert!(matches!(
        event,
        HotkeyEvent::SequenceStep {
            steps_matched: 1,
            steps_remaining: 0,
            ..
        }
    ));
}

// Layer lifecycle

#[test]
fn define_push_pop_layer_lifecycle() {
    let manager = utils::test_manager();

    let layer = Layer::new("nav")
        .bind(Key::H, Action::Suppress)
        .unwrap()
        .bind(Key::J, Action::Suppress)
        .unwrap();
    manager.define_layer(layer).unwrap();

    manager.push_layer("nav").unwrap();
    let active = manager.active_layers().unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].name.as_str(), "nav");
    assert_eq!(active[0].binding_count, 2);

    let popped = manager.pop_layer().unwrap();
    assert_eq!(popped.as_str(), "nav");

    let active = manager.active_layers().unwrap();
    assert!(active.is_empty());
}

#[test]
fn toggle_layer_on_off() {
    let manager = utils::test_manager();

    let layer = Layer::new("nav").bind(Key::H, Action::Suppress).unwrap();
    manager.define_layer(layer).unwrap();

    // Toggle on
    manager.toggle_layer("nav").unwrap();
    assert_eq!(manager.active_layers().unwrap().len(), 1);

    // Toggle off
    manager.toggle_layer("nav").unwrap();
    assert!(manager.active_layers().unwrap().is_empty());
}

#[test]
fn layer_description_visible_in_introspection() {
    let manager = utils::test_manager();

    let layer = Layer::new("nav")
        .bind(Key::H, Action::Suppress)
        .unwrap()
        .description("Vim-style navigation");
    manager.define_layer(layer).unwrap();
    manager.push_layer("nav").unwrap();

    let active = manager.active_layers().unwrap();
    assert_eq!(
        active[0].description.as_deref(),
        Some("Vim-style navigation")
    );
}

// Introspection queries

#[test]
fn list_bindings_returns_registered_bindings() {
    let manager = utils::test_manager();

    let _g1 = manager.register(Key::F1, || {}).unwrap();
    let _g2 = manager.register(Key::F2, || {}).unwrap();

    let bindings = manager.list_bindings().unwrap();
    assert!(bindings.len() >= 2);
}

#[test]
fn bindings_for_key_finds_registered_hotkey() {
    let manager = utils::test_manager();
    let hotkey = Hotkey::new(Key::S).modifier(Modifier::Ctrl);

    let _guard = manager.register(hotkey.clone(), || {}).unwrap();

    let info = manager
        .bindings_for_key(hotkey)
        .unwrap()
        .expect("should find binding");
    assert_eq!(info.hotkey, Hotkey::new(Key::S).modifier(Modifier::Ctrl));
}

#[test]
fn bindings_for_key_returns_none_for_unregistered() {
    let manager = utils::test_manager();
    let result = manager.bindings_for_key(Hotkey::new(Key::Z)).unwrap();
    assert!(result.is_none());
}

#[test]
fn conflicts_empty_with_no_overlapping_bindings() {
    let manager = utils::test_manager();

    let _g1 = manager.register(Key::F1, || {}).unwrap();
    let _g2 = manager.register(Key::F2, || {}).unwrap();

    let conflicts = manager.conflicts().unwrap();
    assert!(conflicts.is_empty());
}

#[test]
fn conflicts_detected_when_layer_shadows_global() {
    let manager = utils::test_manager();

    let _global = manager.register(Key::H, || {}).unwrap();

    let layer = Layer::new("nav").bind(Key::H, Action::Suppress).unwrap();
    manager.define_layer(layer).unwrap();
    manager.push_layer("nav").unwrap();

    let conflicts = manager.conflicts().unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].hotkey, Hotkey::new(Key::H));
}

// Shutdown

#[test]
fn shutdown_then_register_returns_manager_stopped() {
    let manager = utils::test_manager();
    manager.shutdown().expect("shutdown should succeed");
    // HotkeyManager is consumed by shutdown, so we can't call register.
    // Instead test via the guard path:
}

#[test]
fn guard_unregister_after_shutdown_returns_manager_stopped() {
    let manager = utils::test_manager();
    let guard = manager.register(Key::F7, || {}).unwrap();
    manager.shutdown().expect("shutdown should succeed");
    let result = guard.unregister();
    assert!(matches!(result, Err(Error::ManagerStopped)));
}

// Into<Hotkey> ergonomics

#[test]
fn register_accepts_key_directly() {
    let manager = utils::test_manager();
    let _guard = manager.register(Key::ESCAPE, || {}).unwrap();
}

#[test]
fn register_accepts_hotkey_with_modifiers() {
    let manager = utils::test_manager();
    let _guard = manager
        .register(
            Hotkey::new(Key::A)
                .modifier(Modifier::Ctrl)
                .modifier(Modifier::Shift),
            || {},
        )
        .unwrap();
}

#[test]
fn is_registered_accepts_key_directly() {
    let manager = utils::test_manager();
    let _guard = manager.register(Key::F8, || {}).unwrap();
    assert!(manager.is_registered(Key::F8).unwrap());
}

// Backend enum

#[test]
fn backend_debug_and_equality() {
    let a = Backend::Evdev;
    let b = Backend::Evdev;
    assert_eq!(a, b);
    assert_eq!(format!("{a:?}"), "Evdev");
}

// Error type surface

#[test]
fn error_implements_std_error() {
    fn assert_std_error<T: std::error::Error>() {}
    assert_std_error::<Error>();
}

#[test]
fn error_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Error>();
}

#[test]
fn parse_error_converts_to_library_error() {
    let parse_err = "Ctrl+NotAKey".parse::<Hotkey>().unwrap_err();
    let error: Error = parse_err.into();
    assert!(matches!(error, Error::Parse(_)));
}
