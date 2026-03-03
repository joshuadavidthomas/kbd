//! Introspection types — snapshots of matcher state for UI and debugging.
//!
//! Every type here is a **read-only snapshot**, safe to hold indefinitely.
//! The matcher's actual state may change after the snapshot is taken.
//!
//! Used by [`Matcher::list_bindings`](crate::Matcher::list_bindings),
//! [`Matcher::bindings_for_key`](crate::Matcher::bindings_for_key),
//! [`Matcher::active_layers`](crate::Matcher::active_layers), and
//! [`Matcher::conflicts`](crate::Matcher::conflicts).
//!
//! # Examples
//!
//! ```
//! use kbd::{Action, Hotkey, Key, Layer, Matcher, Modifier};
//! use kbd::{BindingLocation, ShadowedStatus};
//!
//! let mut matcher = Matcher::new();
//! matcher.register(
//!     Hotkey::new(Key::C).modifier(Modifier::Ctrl),
//!     Action::Suppress,
//! ).unwrap();
//!
//! let bindings = matcher.list_bindings();
//! assert_eq!(bindings.len(), 1);
//! assert_eq!(bindings[0].location, BindingLocation::Global);
//! assert_eq!(bindings[0].shadowed, ShadowedStatus::Active);
//! ```

use crate::action::LayerName;
use crate::binding::OverlayVisibility;
use crate::key::Hotkey;

/// Where a binding lives in the registration hierarchy.
///
/// Returned as part of [`BindingInfo`] from matcher introspection methods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingLocation {
    /// Registered globally (always active, checked after layers).
    Global,
    /// Registered within a named layer.
    Layer(LayerName),
}

/// Whether a binding is currently reachable or shadowed.
///
/// When a layer is active and contains a binding for the same hotkey as a
/// global or lower-layer binding, the higher-priority binding "shadows"
/// the lower one.
///
/// # Examples
///
/// ```
/// use kbd::{Action, Hotkey, Key, Layer, Matcher, Modifier};
/// use kbd::ShadowedStatus;
///
/// let mut matcher = Matcher::new();
/// matcher.register(Hotkey::new(Key::H), Action::Suppress).unwrap();
///
/// // Define a layer that also binds H
/// matcher.define_layer(
///     Layer::new("nav").bind(Key::H, Action::Suppress)
/// ).unwrap();
/// matcher.push_layer("nav").unwrap();
///
/// // The global H binding is now shadowed by the nav layer
/// let bindings = matcher.list_bindings();
/// let global_h = bindings.iter()
///     .find(|b| b.location == kbd::BindingLocation::Global)
///     .unwrap();
/// assert!(matches!(global_h.shadowed, ShadowedStatus::ShadowedBy(_)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShadowedStatus {
    /// This binding would fire if its hotkey were pressed now.
    Active,
    /// A higher-priority layer has a binding with the same hotkey.
    ShadowedBy(LayerName),
    /// This binding's layer is not currently on the stack.
    Inactive,
}

/// Snapshot of a single binding for introspection.
///
/// Returned by [`Matcher::list_bindings`](crate::Matcher::list_bindings) and
/// [`Matcher::bindings_for_key`](crate::Matcher::bindings_for_key).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingInfo {
    /// The hotkey (key + modifiers) that triggers this binding.
    pub hotkey: Hotkey,
    /// Human-readable label, if one was set via [`BindingOptions`](crate::BindingOptions).
    pub description: Option<Box<str>>,
    /// Where this binding lives (global or a specific layer).
    pub location: BindingLocation,
    /// Whether this binding is currently reachable or shadowed.
    pub shadowed: ShadowedStatus,
    /// Whether this binding appears in hotkey overlays.
    pub overlay_visibility: OverlayVisibility,
}

/// Snapshot of an active layer on the stack.
///
/// Returned by [`Matcher::active_layers`](crate::Matcher::active_layers).
///
/// # Examples
///
/// ```
/// use kbd::{Action, Key, Layer, Matcher};
///
/// let mut matcher = Matcher::new();
/// matcher.define_layer(
///     Layer::new("nav")
///         .bind(Key::H, Action::Suppress)
///         .bind(Key::J, Action::Suppress)
///         .description("Navigation keys")
/// ).unwrap();
/// matcher.push_layer("nav").unwrap();
///
/// let layers = matcher.active_layers();
/// assert_eq!(layers.len(), 1);
/// assert_eq!(layers[0].name.as_str(), "nav");
/// assert_eq!(layers[0].description.as_deref(), Some("Navigation keys"));
/// assert_eq!(layers[0].binding_count, 2);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveLayerInfo {
    /// The layer's name.
    pub name: LayerName,
    /// Human-readable label, if one was set on the layer.
    pub description: Option<Box<str>>,
    /// Number of bindings defined in this layer.
    pub binding_count: usize,
}

/// A pair of bindings in conflict — one shadows the other.
///
/// Returned by [`Matcher::conflicts`](crate::Matcher::conflicts).
///
/// # Examples
///
/// ```
/// use kbd::{Action, Hotkey, Key, Layer, Matcher};
///
/// let mut matcher = Matcher::new();
/// matcher.register(Hotkey::new(Key::H), Action::Suppress).unwrap();
/// matcher.define_layer(
///     Layer::new("nav").bind(Key::H, Action::Suppress)
/// ).unwrap();
/// matcher.push_layer("nav").unwrap();
///
/// let conflicts = matcher.conflicts();
/// assert_eq!(conflicts.len(), 1);
/// assert_eq!(conflicts[0].hotkey, Hotkey::new(Key::H));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictInfo {
    /// The hotkey at the center of the conflict.
    pub hotkey: Hotkey,
    /// The binding that is being shadowed (lower priority).
    pub shadowed_binding: BindingInfo,
    /// The binding that is doing the shadowing (higher priority).
    pub shadowing_binding: BindingInfo,
}
