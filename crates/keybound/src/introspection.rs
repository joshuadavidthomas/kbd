//! Introspection types — snapshots of engine state for UI and debugging.
//!
//! Every type here is a **read-only snapshot**, safe to hold indefinitely.
//! The engine's actual state may change after the snapshot is taken.
//!
//! Used by `HotkeyManager::list_bindings()`, `bindings_for_key()`,
//! `active_layers()`, and `conflicts()`.

use crate::action::LayerName;
use crate::binding::OverlayVisibility;
use crate::key::Hotkey;

/// Where a binding lives in the registration hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingLocation {
    /// Registered globally (always active, checked after layers).
    Global,
    /// Registered within a named layer.
    Layer(LayerName),
}

/// Whether a binding is currently reachable or shadowed.
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
/// Returned by [`HotkeyManager::list_bindings`] and
/// [`HotkeyManager::bindings_for_key`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingInfo {
    /// The hotkey (key + modifiers) that triggers this binding.
    pub hotkey: Hotkey,
    /// Human-readable label, if one was set via `BindingOptions`.
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
/// Returned by [`HotkeyManager::active_layers`].
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
/// Returned by [`HotkeyManager::conflicts`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictInfo {
    /// The hotkey at the center of the conflict.
    pub hotkey: Hotkey,
    /// The binding that is being shadowed (lower priority).
    pub shadowed_binding: BindingInfo,
    /// The binding that is doing the shadowing (higher priority).
    pub shadowing_binding: BindingInfo,
}
