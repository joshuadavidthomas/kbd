//! Introspection types — snapshots of dispatcher state for UI and debugging.
//!
//! Every type here is a **read-only snapshot**, safe to hold indefinitely.
//! The dispatcher's actual state may change after the snapshot is taken.
//!
//! Used by [`Dispatcher::list_bindings`](crate::dispatcher::Dispatcher::list_bindings),
//! [`Dispatcher::bindings_for_key`](crate::dispatcher::Dispatcher::bindings_for_key),
//! [`Dispatcher::active_layers`](crate::dispatcher::Dispatcher::active_layers), and
//! [`Dispatcher::conflicts`](crate::dispatcher::Dispatcher::conflicts).
//!
//! # Examples
//!
//! ```
//! use kbd::action::Action;
//! use kbd::dispatcher::Dispatcher;
//! use kbd::introspection::{BindingLocation, ShadowedStatus};
//! use kbd::hotkey::{Hotkey, Modifier};
//! use kbd::key::Key;
//! use kbd::layer::Layer;
//!
//! # fn main() -> Result<(), kbd::error::Error> {
//! let mut dispatcher = Dispatcher::new();
//! dispatcher.register(
//!     Hotkey::new(Key::C).modifier(Modifier::Ctrl),
//!     Action::Suppress,
//! )?;
//!
//! let bindings = dispatcher.list_bindings();
//! assert_eq!(bindings.len(), 1);
//! assert_eq!(bindings[0].location, BindingLocation::Global);
//! assert_eq!(bindings[0].shadowed, ShadowedStatus::Active);
//! # Ok(())
//! # }
//! ```

use crate::binding::OverlayVisibility;
use crate::hotkey::Hotkey;
use crate::layer::LayerName;

/// Where a binding lives in the registration hierarchy.
///
/// Returned as part of [`BindingInfo`] from dispatcher introspection methods.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
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
/// use kbd::action::Action;
/// use kbd::dispatcher::Dispatcher;
/// use kbd::introspection::ShadowedStatus;
/// use kbd::hotkey::{Hotkey, Modifier};
/// use kbd::key::Key;
/// use kbd::layer::Layer;
///
/// # fn main() -> Result<(), kbd::error::Error> {
/// let mut dispatcher = Dispatcher::new();
/// dispatcher.register(Hotkey::new(Key::H), Action::Suppress)?;
///
/// // Define a layer that also binds H
/// dispatcher.define_layer(
///     Layer::new("nav").bind(Key::H, Action::Suppress)?
/// )?;
/// dispatcher.push_layer("nav")?;
///
/// // The global H binding is now shadowed by the nav layer
/// let bindings = dispatcher.list_bindings();
/// let global_h = bindings.iter()
///     .find(|b| b.location == kbd::introspection::BindingLocation::Global)
///     .expect("just registered a global binding");
/// assert!(matches!(global_h.shadowed, ShadowedStatus::ShadowedBy(_)));
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
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
/// Returned by [`Dispatcher::list_bindings`](crate::dispatcher::Dispatcher::list_bindings) and
/// [`Dispatcher::bindings_for_key`](crate::dispatcher::Dispatcher::bindings_for_key).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingInfo {
    /// The hotkey (key + modifiers) that triggers this binding.
    pub hotkey: Hotkey,
    /// Human-readable label, if one was set via [`BindingOptions`](crate::binding::BindingOptions).
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
/// Returned by [`Dispatcher::active_layers`](crate::dispatcher::Dispatcher::active_layers).
///
/// # Examples
///
/// ```
/// use kbd::action::Action;
/// use kbd::dispatcher::Dispatcher;
/// use kbd::key::Key;
/// use kbd::layer::Layer;
///
/// # fn main() -> Result<(), kbd::error::Error> {
/// let mut dispatcher = Dispatcher::new();
/// dispatcher.define_layer(
///     Layer::new("nav")
///         .bind(Key::H, Action::Suppress)?
///         .bind(Key::J, Action::Suppress)?
///         .description("Navigation keys")
/// )?;
/// dispatcher.push_layer("nav")?;
///
/// let layers = dispatcher.active_layers();
/// assert_eq!(layers.len(), 1);
/// assert_eq!(layers[0].name.as_str(), "nav");
/// assert_eq!(layers[0].description.as_deref(), Some("Navigation keys"));
/// assert_eq!(layers[0].binding_count, 2);
/// # Ok(())
/// # }
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
/// Returned by [`Dispatcher::conflicts`](crate::dispatcher::Dispatcher::conflicts).
///
/// # Examples
///
/// ```
/// use kbd::action::Action;
/// use kbd::dispatcher::Dispatcher;
/// use kbd::hotkey::Hotkey;
/// use kbd::key::Key;
/// use kbd::layer::Layer;
///
/// # fn main() -> Result<(), kbd::error::Error> {
/// let mut dispatcher = Dispatcher::new();
/// dispatcher.register(Hotkey::new(Key::H), Action::Suppress)?;
/// dispatcher.define_layer(
///     Layer::new("nav").bind(Key::H, Action::Suppress)?
/// )?;
/// dispatcher.push_layer("nav")?;
///
/// let conflicts = dispatcher.conflicts();
/// assert_eq!(conflicts.len(), 1);
/// assert_eq!(conflicts[0].hotkey, Hotkey::new(Key::H));
/// # Ok(())
/// # }
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
