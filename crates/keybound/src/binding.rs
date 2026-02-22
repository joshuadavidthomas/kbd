//! The unified [`Binding`] type — pattern + action + options.
//!
//! A binding is the core unit: "when this input pattern matches, do this
//! action." Replaces the four separate registration types from v0:
//!
//! | v0 type                      | Now expressed as              |
//! |------------------------------|-------------------------------|
//! | `HotkeyRegistration`         | Binding with Hotkey pattern   |
//! | `SequenceRegistration`       | Binding with Sequence pattern |
//! | `DeviceHotkeyRegistration`   | Binding with device filter    |
//! | `TapHoldRegistration`        | Binding with `TapHold` pattern  |
//!
//! One storage structure. One dispatch path. One handle type.
//!
//! # Reference
//!
//! Prior art: `archive/v0/src/manager/registration.rs`,
//! `archive/v0/src/listener/dispatch.rs`

use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

/// Unique identifier for a registered binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BindingId(u64);

impl BindingId {
    /// Create a new globally unique binding ID.
    #[must_use]
    pub fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }

    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl Default for BindingId {
    fn default() -> Self {
        Self::new()
    }
}

/// How a matched binding handles the original key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Passthrough {
    /// Consume the event by default.
    #[default]
    Consume,
    /// Forward the event while still running the action.
    Enabled,
}

/// Whether a binding appears in hotkey overlays and help screens.
///
/// Lets consumers build discoverable hotkey overlays while excluding
/// internal or administrative bindings. Follows the pattern from
/// Niri's `hotkey-overlay-title=null`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum OverlayVisibility {
    /// Binding is shown in overlays and help screens.
    #[default]
    Visible,
    /// Binding is hidden from overlays and help screens.
    Hidden,
}

/// Device filter expression for restricting binding scope.
#[cfg(feature = "evdev")]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DeviceFilter {
    /// Match devices whose names fit a glob-like pattern.
    NamePattern(Box<str>),
    /// Match devices by USB vendor/product IDs.
    Usb { vendor_id: u16, product_id: u16 },
}

/// Per-binding behavioral options.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct BindingOptions {
    passthrough: Passthrough,
    /// Human-readable label for this binding ("Copy to clipboard").
    description: Option<Box<str>>,
    /// Whether this binding appears in hotkey overlays and help screens.
    overlay_visibility: OverlayVisibility,
    #[cfg(feature = "evdev")]
    device_filter: Option<DeviceFilter>,
}

impl BindingOptions {
    #[must_use]
    pub const fn passthrough(&self) -> Passthrough {
        self.passthrough
    }

    #[must_use]
    pub const fn with_passthrough(mut self, passthrough: Passthrough) -> Self {
        self.passthrough = passthrough;
        self
    }

    /// Human-readable label for this binding, if set.
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Set a human-readable label for this binding.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<Box<str>>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Whether this binding appears in hotkey overlays.
    #[must_use]
    pub const fn overlay_visibility(&self) -> OverlayVisibility {
        self.overlay_visibility
    }

    /// Set overlay visibility for this binding.
    #[must_use]
    pub const fn with_overlay_visibility(mut self, visibility: OverlayVisibility) -> Self {
        self.overlay_visibility = visibility;
        self
    }

    #[cfg(feature = "evdev")]
    #[must_use]
    pub fn with_device_filter(mut self, device_filter: DeviceFilter) -> Self {
        self.device_filter = Some(device_filter);
        self
    }

    #[cfg(feature = "evdev")]
    #[must_use]
    pub fn device_filter(&self) -> Option<&DeviceFilter> {
        self.device_filter.as_ref()
    }
}
