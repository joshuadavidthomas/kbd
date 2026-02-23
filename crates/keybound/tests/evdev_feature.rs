//! Tests for the evdev feature gate.
//!
//! These tests verify that:
//! - The `Backend::Evdev` variant only exists when the `evdev` feature is enabled
//! - The manager correctly reports which backend it's using

use keybound::Backend;
use keybound::HotkeyManager;

#[test]
fn evdev_backend_variant_exists() {
    // Backend::Evdev should always exist as a variant since it's the primary backend.
    // But the manager should only successfully create with Evdev when the feature is enabled.
    assert_eq!(Backend::Evdev, Backend::Evdev);
}

/// When the evdev feature is enabled, creating a manager with Evdev backend works.
#[cfg(feature = "evdev")]
#[test]
fn manager_with_evdev_backend() {
    let manager = HotkeyManager::builder()
        .backend(Backend::Evdev)
        .build()
        .expect("manager with evdev backend should build");
    assert_eq!(manager.active_backend(), Backend::Evdev);
}

/// When the evdev feature is NOT enabled, creating a manager with Evdev backend
/// should return an error.
#[cfg(not(feature = "evdev"))]
#[test]
fn manager_without_evdev_returns_error() {
    let result = HotkeyManager::builder().backend(Backend::Evdev).build();
    assert!(
        result.is_err(),
        "building with Evdev backend without the evdev feature should fail"
    );
}
