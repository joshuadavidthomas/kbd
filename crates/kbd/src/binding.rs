//! The unified binding types — pattern + action + options.
//!
//! A binding is the core unit: "when this input pattern matches, do this
//! action." [`BindingId`] uniquely identifies a binding. [`BindingOptions`]
//! holds per-binding configuration. [`RegisteredBinding`] pairs them with
//! a hotkey and action for engine storage.
//!
//! Device filtering ([`DeviceFilter`]) is a pure data type describing
//! device match criteria (name patterns, USB IDs).

use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use crate::action::Action;
use crate::key::Hotkey;

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

    /// Return the raw `u64` value of this ID.
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DeviceFilter {
    /// Match devices whose names fit a glob-like pattern.
    NamePattern(Box<str>),
    /// Match devices by USB vendor/product IDs.
    Usb {
        /// USB vendor ID.
        vendor_id: u16,
        /// USB product ID.
        product_id: u16,
    },
}

/// Per-binding behavioral options.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct BindingOptions {
    passthrough: Passthrough,
    /// Human-readable label for this binding ("Copy to clipboard").
    description: Option<Box<str>>,
    /// Whether this binding appears in hotkey overlays and help screens.
    overlay_visibility: OverlayVisibility,
    device_filter: Option<DeviceFilter>,
}

impl BindingOptions {
    /// How the original key event is handled after matching.
    #[must_use]
    pub const fn passthrough(&self) -> Passthrough {
        self.passthrough
    }

    /// Set the passthrough behavior.
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

    /// Restrict this binding to a specific input device.
    #[must_use]
    pub fn with_device_filter(mut self, device_filter: DeviceFilter) -> Self {
        self.device_filter = Some(device_filter);
        self
    }

    /// The device filter for this binding, if set.
    #[must_use]
    pub fn device_filter(&self) -> Option<&DeviceFilter> {
        self.device_filter.as_ref()
    }
}

/// A binding registered with the engine: hotkey + action + options.
///
/// This is the engine's storage type for bindings. Created by the manager
/// and sent to the engine via command channel.
pub struct RegisteredBinding {
    id: BindingId,
    hotkey: Hotkey,
    action: Action,
    options: BindingOptions,
}

impl RegisteredBinding {
    /// Create a registered binding with default options.
    #[must_use]
    pub fn new(id: BindingId, hotkey: Hotkey, action: Action) -> Self {
        Self {
            id,
            hotkey,
            action,
            options: BindingOptions::default(),
        }
    }

    /// Replace the binding's options.
    #[must_use]
    pub fn with_options(mut self, options: BindingOptions) -> Self {
        self.options = options;
        self
    }

    /// Set the passthrough behavior for this binding.
    #[must_use]
    pub fn with_passthrough(mut self, passthrough: Passthrough) -> Self {
        self.options = self.options.with_passthrough(passthrough);
        self
    }

    /// The unique ID of this binding.
    #[must_use]
    pub const fn id(&self) -> BindingId {
        self.id
    }

    /// The hotkey pattern that triggers this binding.
    #[must_use]
    pub fn hotkey(&self) -> &Hotkey {
        &self.hotkey
    }

    /// The action to execute when this binding matches.
    #[must_use]
    pub const fn action(&self) -> &Action {
        &self.action
    }

    /// How the original key event is handled after matching.
    #[must_use]
    pub const fn passthrough(&self) -> Passthrough {
        self.options.passthrough()
    }

    /// The full options for this binding.
    #[must_use]
    pub const fn options(&self) -> &BindingOptions {
        &self.options
    }
}
