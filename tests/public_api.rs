//! Integration tests for the public API surface (Phase 1.8).
//!
//! Verifies that the public re-exports are correct, minimal, and usable.
//! Verifies that the DESIGN.md example compiles and works.

use keybound::Action;
use keybound::Backend;
use keybound::BindingId;
use keybound::BindingOptions;
use keybound::DeviceFilter;
use keybound::Error;
use keybound::Handle;
use keybound::Hotkey;
use keybound::HotkeyManager;
use keybound::HotkeyManagerBuilder;
use keybound::HotkeySequence;
use keybound::Key;
use keybound::Layer;
use keybound::LayerName;
use keybound::LayerOptions;
use keybound::Modifier;
use keybound::ParseHotkeyError;
use keybound::Passthrough;

// The DESIGN.md simple example:
//
//   use keybound::{HotkeyManager, Key, Modifier};
//   let manager = HotkeyManager::new()?;
//   let _handle = manager.register(
//       Key::C, &[Modifier::Ctrl, Modifier::Shift],
//       || println!("fired"),
//   )?;
//
#[test]
fn design_md_simple_example() {
    let manager: HotkeyManager = HotkeyManager::new().expect("manager should start");
    let handle: Handle = manager
        .register(Key::C, &[Modifier::Ctrl, Modifier::Shift], || {
            println!("fired");
        })
        .expect("registration should succeed");
    drop(handle);
}

#[test]
fn key_types_are_usable_through_public_api() {
    // Key enum variants
    let _ = Key::A;
    let _ = Key::LeftCtrl;
    let _ = Key::F1;

    // Modifier enum variants
    let _ = Modifier::Ctrl;
    let _ = Modifier::Shift;
    let _ = Modifier::Alt;
    let _ = Modifier::Super;

    // Hotkey from parsing
    let hotkey: Hotkey = "Ctrl+A".parse().expect("should parse");
    assert_eq!(hotkey.to_string(), "Ctrl+A");

    // HotkeySequence from parsing
    let seq: HotkeySequence = "Ctrl+K, Ctrl+C".parse().expect("should parse");
    assert_eq!(seq.to_string(), "Ctrl+K, Ctrl+C");
}

#[test]
fn action_from_closure_conversion() {
    // Closures auto-convert to Action via From
    let action: Action = Action::from(|| println!("hello"));
    assert!(matches!(action, Action::Callback(_)));

    // Other action variants exist
    let _ = Action::Swallow;
    let _ = Action::PopLayer;
    let _ = Action::PushLayer(LayerName::new("test"));
    let _ = Action::ToggleLayer(LayerName::new("test"));
    let _ = Action::EmitKey(Key::A, vec![]);
    let _ = Action::EmitSequence("A".parse().unwrap());
}

#[test]
fn binding_types_are_usable() {
    // BindingId is constructable and comparable
    let id1 = BindingId::new();
    let id2 = BindingId::new();
    assert_ne!(id1, id2);

    // BindingOptions has builder-style API
    let opts = BindingOptions::default();
    assert_eq!(opts.passthrough(), Passthrough::Consume);

    let opts = opts.with_passthrough(Passthrough::Enabled);
    assert_eq!(opts.passthrough(), Passthrough::Enabled);

    // DeviceFilter variants
    let _ = DeviceFilter::NamePattern("keyboard*".into());
    let _ = DeviceFilter::Usb {
        vendor_id: 0x1234,
        product_id: 0x5678,
    };
}

#[test]
fn error_types_are_usable() {
    // Error variants exist and are Display
    let err = Error::AlreadyRegistered;
    assert!(!err.to_string().is_empty());

    // ParseHotkeyError is convertible to Error
    let parse_result: Result<Hotkey, ParseHotkeyError> = "not-a-key+++".parse();
    assert!(parse_result.is_err());
    let err: Error = parse_result.unwrap_err().into();
    assert!(!err.to_string().is_empty());
}

#[test]
fn manager_builder_api() {
    // Builder pattern works
    let builder: HotkeyManagerBuilder = HotkeyManager::builder();
    let manager = builder.build().expect("should build");
    assert_eq!(manager.active_backend(), Backend::Evdev);
    manager.shutdown().expect("shutdown should succeed");
}

#[test]
fn placeholder_types_are_exported() {
    // Layer and LayerOptions are placeholder types for forward compatibility.
    // They exist in the public API surface now but are implemented in Phase 3.
    let _layer = Layer;
    let _opts = LayerOptions;
}

#[test]
fn register_multiple_non_conflicting_hotkeys() {
    let manager = HotkeyManager::new().expect("manager should start");

    let h1 = manager
        .register(Key::A, &[Modifier::Ctrl], || {})
        .expect("first should register");

    let h2 = manager
        .register(Key::B, &[Modifier::Ctrl], || {})
        .expect("second should register");

    let h3 = manager
        .register(Key::A, &[Modifier::Ctrl, Modifier::Shift], || {})
        .expect("third should register (different modifiers)");

    // All are registered
    assert!(manager.is_registered(Key::A, &[Modifier::Ctrl]).unwrap());
    assert!(manager.is_registered(Key::B, &[Modifier::Ctrl]).unwrap());
    assert!(manager
        .is_registered(Key::A, &[Modifier::Ctrl, Modifier::Shift])
        .unwrap());

    // Drop handles
    drop(h1);
    drop(h2);
    drop(h3);
}

#[test]
fn handle_provides_binding_id() {
    let manager = HotkeyManager::new().expect("manager should start");
    let handle = manager
        .register(Key::X, &[], || {})
        .expect("should register");

    // binding_id() is accessible
    let _id: BindingId = handle.binding_id();
}
