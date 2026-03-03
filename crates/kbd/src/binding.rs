//! The unified binding types — pattern + action + options.
//!
//! A binding is the core unit: "when this input pattern matches, do this
//! action." [`BindingId`](crate::binding::BindingId) uniquely identifies a
//! binding. [`BindingOptions`](crate::binding::BindingOptions) holds
//! per-binding configuration.
//! [`RegisteredBinding`](crate::binding::RegisteredBinding) pairs them with
//! a hotkey and action for engine storage.

use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use crate::action::Action;
use crate::hotkey::Hotkey;

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
///
/// # Examples
///
/// ```
/// use kbd::action::Action;
/// use kbd::binding::{BindingId, BindingOptions, KeyPropagation, RegisteredBinding};
/// use kbd::hotkey::{Hotkey, Modifier};
/// use kbd::key::Key;
///
/// // A binding that forwards the key event to the application
/// // while still running its action (e.g., logging keypresses).
/// let binding = RegisteredBinding::new(
///     BindingId::new(),
///     Hotkey::new(Key::S).modifier(Modifier::Ctrl),
///     Action::Suppress,
/// ).with_propagation(KeyPropagation::Continue);
///
/// assert_eq!(binding.propagation(), KeyPropagation::Continue);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum KeyPropagation {
    /// Stop propagation — the event is consumed and not forwarded.
    #[default]
    Stop,
    /// Continue propagation — forward the event while still running the action.
    Continue,
}

/// Whether a binding appears in hotkey overlays and help screens.
///
/// Lets consumers build discoverable hotkey overlays while excluding
/// internal or administrative bindings. Follows the pattern from
/// Niri's `hotkey-overlay-title=null`.
///
/// # Examples
///
/// ```
/// use kbd::binding::{BindingOptions, OverlayVisibility};
///
/// // Hide an internal binding from the overlay
/// let opts = BindingOptions::default()
///     .with_overlay_visibility(OverlayVisibility::Hidden);
/// assert_eq!(opts.overlay_visibility(), OverlayVisibility::Hidden);
///
/// // By default, bindings are visible
/// let opts = BindingOptions::default();
/// assert_eq!(opts.overlay_visibility(), OverlayVisibility::Visible);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum OverlayVisibility {
    /// Binding is shown in overlays and help screens.
    #[default]
    Visible,
    /// Binding is hidden from overlays and help screens.
    Hidden,
}

/// Per-binding behavioral options.
///
/// Configure a binding's key propagation behavior, description, and overlay
/// visibility. Built via method chaining:
///
/// # Examples
///
/// ```
/// use kbd::binding::{BindingOptions, KeyPropagation, OverlayVisibility};
///
/// let opts = BindingOptions::default()
///     .with_description("Copy to clipboard")
///     .with_propagation(KeyPropagation::Stop)
///     .with_overlay_visibility(OverlayVisibility::Visible);
///
/// assert_eq!(opts.description(), Some("Copy to clipboard"));
/// assert_eq!(opts.propagation(), KeyPropagation::Stop);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct BindingOptions {
    propagation: KeyPropagation,
    /// Human-readable label for this binding ("Copy to clipboard").
    description: Option<Box<str>>,
    /// Whether this binding appears in hotkey overlays and help screens.
    overlay_visibility: OverlayVisibility,
}

impl BindingOptions {
    /// How the original key event is handled after matching.
    #[must_use]
    pub const fn propagation(&self) -> KeyPropagation {
        self.propagation
    }

    /// Set the key propagation behavior.
    #[must_use]
    pub const fn with_propagation(mut self, propagation: KeyPropagation) -> Self {
        self.propagation = propagation;
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

    /// Set the key propagation behavior for this binding.
    #[must_use]
    pub fn with_propagation(mut self, propagation: KeyPropagation) -> Self {
        self.options = self.options.with_propagation(propagation);
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
    pub const fn propagation(&self) -> KeyPropagation {
        self.options.propagation()
    }

    /// The full options for this binding.
    #[must_use]
    pub const fn options(&self) -> &BindingOptions {
        &self.options
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hotkey::Modifier;
    use crate::key::Key;

    #[test]
    fn binding_id_produces_unique_ids() {
        let a = BindingId::new();
        let b = BindingId::new();
        let c = BindingId::new();
        assert_ne!(a, b);
        assert_ne!(b, c);
    }

    #[test]
    fn binding_id_monotonically_increases() {
        let a = BindingId::new();
        let b = BindingId::new();
        assert!(b.as_u64() > a.as_u64());
    }

    #[test]
    fn binding_id_default_calls_new() {
        let a = BindingId::default();
        let b = BindingId::default();
        assert_ne!(a, b);
    }

    #[test]
    fn binding_options_defaults() {
        let opts = BindingOptions::default();
        assert_eq!(opts.propagation(), KeyPropagation::Stop);
        assert_eq!(opts.description(), None);
        assert_eq!(opts.overlay_visibility(), OverlayVisibility::Visible);
    }

    #[test]
    fn binding_options_builder_chain() {
        let opts = BindingOptions::default()
            .with_propagation(KeyPropagation::Continue)
            .with_description("Save file")
            .with_overlay_visibility(OverlayVisibility::Hidden);

        assert_eq!(opts.propagation(), KeyPropagation::Continue);
        assert_eq!(opts.description(), Some("Save file"));
        assert_eq!(opts.overlay_visibility(), OverlayVisibility::Hidden);
    }

    #[test]
    fn registered_binding_stores_fields() {
        let id = BindingId::new();
        let hotkey = Hotkey::new(Key::S).modifier(Modifier::Ctrl);
        let binding = RegisteredBinding::new(id, hotkey.clone(), Action::Suppress);

        assert_eq!(binding.id(), id);
        assert_eq!(*binding.hotkey(), hotkey);
        assert_eq!(binding.propagation(), KeyPropagation::Stop);
    }

    #[test]
    fn registered_binding_with_propagation() {
        let id = BindingId::new();
        let binding = RegisteredBinding::new(id, Hotkey::new(Key::A), Action::Suppress)
            .with_propagation(KeyPropagation::Continue);

        assert_eq!(binding.propagation(), KeyPropagation::Continue);
    }

    #[test]
    fn registered_binding_with_options() {
        let id = BindingId::new();
        let opts = BindingOptions::default()
            .with_description("test")
            .with_overlay_visibility(OverlayVisibility::Hidden);
        let binding =
            RegisteredBinding::new(id, Hotkey::new(Key::A), Action::Suppress).with_options(opts);

        assert_eq!(binding.options().description(), Some("test"));
        assert_eq!(
            binding.options().overlay_visibility(),
            OverlayVisibility::Hidden
        );
    }
}
