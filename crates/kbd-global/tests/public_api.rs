//! Integration tests for the public API surface (Phase 1.8).
//!
//! The DESIGN.md example compiles and runs. The builder API works.
//!
//! Behavioral coverage for individual types (keys, actions, bindings,
//! errors) lives in their own test files (`action_binding.rs`,
//! `error_type.rs`, `key_types.rs`, `string_parsing.rs`, `manager_handle.rs`).

use kbd_global::Backend;
use kbd_global::Handle;
use kbd_global::Hotkey;
use kbd_global::HotkeyManager;
use kbd_global::Key;
use kbd_global::Modifier;

#[test]
fn design_md_simple_example() {
    let manager = HotkeyManager::new().expect("manager should start");
    let _handle: Handle = manager
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
fn manager_builder_api() {
    let manager = HotkeyManager::builder().build().expect("should build");
    assert_eq!(manager.active_backend(), Backend::Evdev);
    manager.shutdown().expect("shutdown should succeed");
}
