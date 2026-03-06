//! The unified binding types — pattern + action + options.
//!
//! A binding is the core unit: "when this input pattern matches, do this
//! action." [`BindingId`] uniquely identifies a
//! binding. [`BindingOptions`] holds
//! per-binding configuration.
//! [`RegisteredBinding`] pairs them with
//! a hotkey and action for engine storage.

use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use crate::action::Action;
use crate::hotkey::Hotkey;

/// Metadata for an input device.
///
/// Platform-agnostic — extracted at device open time and used for
/// [`DeviceFilter`] matching. The device name, USB vendor ID, and USB
/// product ID are the three attributes that identify most devices.
///
/// # Examples
///
/// ```
/// use kbd::binding::DeviceInfo;
///
/// let info = DeviceInfo::new("Elgato StreamDeck XL", 0x0fd9, 0x006c);
/// assert_eq!(info.name(), "Elgato StreamDeck XL");
/// assert_eq!(info.vendor(), 0x0fd9);
/// assert_eq!(info.product(), 0x006c);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeviceInfo {
    name: Box<str>,
    vendor: u16,
    product: u16,
}

impl DeviceInfo {
    /// Create device info from name, vendor ID, and product ID.
    #[must_use]
    pub fn new(name: impl Into<Box<str>>, vendor: u16, product: u16) -> Self {
        Self {
            name: name.into(),
            vendor,
            product,
        }
    }

    /// The device name as reported by the kernel.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// USB vendor ID.
    #[must_use]
    pub const fn vendor(&self) -> u16 {
        self.vendor
    }

    /// USB product ID.
    #[must_use]
    pub const fn product(&self) -> u16 {
        self.product
    }
}

/// Filter for restricting bindings to specific input devices.
///
/// Attach to a binding via [`BindingOptions::with_device`] to make
/// it fire only when the key event comes from a matching device.
///
/// When a binding has a device filter, modifier isolation applies:
/// only modifiers held on the same device count toward matching.
/// Global bindings (no device filter) continue to use aggregate
/// modifier state across all devices.
///
/// # Examples
///
/// ```
/// use kbd::binding::{BindingOptions, DeviceFilter, DeviceInfo};
///
/// // Match by device name substring
/// let by_name = DeviceFilter::name_contains("StreamDeck");
///
/// // Match by USB vendor/product ID
/// let by_id = DeviceFilter::usb(0x0fd9, 0x006c);
///
/// // Attach to binding options
/// let opts = BindingOptions::default()
///     .with_device(by_name);
///
/// // Check matching
/// let info = DeviceInfo::new("Elgato StreamDeck XL", 0x0fd9, 0x006c);
/// assert!(DeviceFilter::name_contains("StreamDeck").matches(&info));
/// assert!(DeviceFilter::usb(0x0fd9, 0x006c).matches(&info));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum DeviceFilter {
    /// Match devices whose name contains the given substring.
    NameContains(Box<str>),
    /// Match devices by USB vendor and product ID.
    UsbId {
        /// USB vendor ID.
        vendor: u16,
        /// USB product ID.
        product: u16,
    },
}

impl DeviceFilter {
    /// Create a filter that matches devices whose name contains the given pattern.
    #[must_use]
    pub fn name_contains(pattern: impl Into<Box<str>>) -> Self {
        Self::NameContains(pattern.into())
    }

    /// Create a filter that matches devices by USB vendor and product ID.
    #[must_use]
    pub fn usb(vendor: u16, product: u16) -> Self {
        Self::UsbId { vendor, product }
    }

    /// Check whether a device matches this filter.
    #[must_use]
    pub fn matches(&self, info: &DeviceInfo) -> bool {
        match self {
            Self::NameContains(pattern) => info.name().contains(pattern.as_ref()),
            Self::UsbId { vendor, product } => {
                info.vendor() == *vendor && info.product() == *product
            }
        }
    }
}

/// Unique identifier for a registered binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
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

/// Provenance label for a binding.
///
/// Tracks where a binding came from — for example `"default"`, `"user"`,
/// `"plugin"`, or an application-specific label.
///
/// The dispatcher recognizes two labels for precedence when multiple global
/// bindings share the same hotkey: `"default"` is lower-priority and `"user"`
/// is higher-priority. Matching is case-insensitive. Other labels — and
/// bindings with no source at all — use the normal priority tier, so there can
/// be at most one binding per hotkey in each tier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct BindingSource(Box<str>);

impl BindingSource {
    /// Create a new source label.
    #[must_use]
    pub fn new(value: impl Into<Box<str>>) -> Self {
        Self(value.into())
    }

    /// Return the source label as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn precedence_tier(&self) -> BindingSourceTier {
        if self.as_str().eq_ignore_ascii_case("default") {
            BindingSourceTier::Default
        } else if self.as_str().eq_ignore_ascii_case("user") {
            BindingSourceTier::User
        } else {
            BindingSourceTier::Standard
        }
    }
}

impl From<&str> for BindingSource {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for BindingSource {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl std::fmt::Display for BindingSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum BindingSourceTier {
    Default,
    Standard,
    User,
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
/// Configure a binding's key propagation behavior, description, source, and
/// overlay visibility. Built via method chaining:
///
/// # Examples
///
/// ```
/// use kbd::binding::{BindingOptions, BindingSource, KeyPropagation, OverlayVisibility};
///
/// let opts = BindingOptions::default()
///     .with_description("Copy to clipboard")
///     .with_source(BindingSource::new("user"))
///     .with_propagation(KeyPropagation::Stop)
///     .with_overlay_visibility(OverlayVisibility::Visible);
///
/// assert_eq!(opts.description(), Some("Copy to clipboard"));
/// assert_eq!(opts.source().map(BindingSource::as_str), Some("user"));
/// assert_eq!(opts.propagation(), KeyPropagation::Stop);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BindingOptions {
    propagation: KeyPropagation,
    /// Human-readable label for this binding ("Copy to clipboard").
    description: Option<Box<str>>,
    /// Provenance label for this binding ("default", "user", "plugin", ...).
    source: Option<BindingSource>,
    /// Whether this binding appears in hotkey overlays and help screens.
    overlay_visibility: OverlayVisibility,
    /// Restrict this binding to a specific device.
    ///
    /// When set, the binding only matches events from devices that pass
    /// the filter. Additionally, per-device modifier isolation applies:
    /// only modifiers held on the matching device count toward matching.
    device: Option<DeviceFilter>,
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

    /// Provenance label for this binding, if set.
    #[must_use]
    pub fn source(&self) -> Option<&BindingSource> {
        self.source.as_ref()
    }

    /// Set a provenance label for this binding.
    ///
    /// Global bindings tagged as `"default"` can be overridden by bindings for
    /// the same hotkey tagged as `"user"` without manually unregistering the
    /// default binding first. Matching is case-insensitive, and labels other
    /// than `"default"`/`"user"` stay in the standard precedence tier.
    #[must_use]
    pub fn with_source(mut self, source: impl Into<BindingSource>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub(crate) fn precedence_tier(&self) -> BindingSourceTier {
        self.source
            .as_ref()
            .map_or(BindingSourceTier::Standard, BindingSource::precedence_tier)
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

    /// The device filter for this binding, if set.
    ///
    /// When a device filter is set, the binding only matches events from
    /// devices that pass the filter, and per-device modifier isolation
    /// applies.
    #[must_use]
    pub fn device(&self) -> Option<&DeviceFilter> {
        self.device.as_ref()
    }

    /// Restrict this binding to events from a specific device.
    ///
    /// When set, the binding only fires for events from devices matching
    /// the filter. Per-device modifier isolation also applies: only
    /// modifiers held on the matching device count toward matching,
    /// not the aggregate modifier state across all devices.
    #[must_use]
    pub fn with_device(mut self, filter: DeviceFilter) -> Self {
        self.device = Some(filter);
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
        assert_eq!(opts.source(), None);
        assert_eq!(opts.overlay_visibility(), OverlayVisibility::Visible);
    }

    #[test]
    fn binding_options_builder_chain() {
        let opts = BindingOptions::default()
            .with_propagation(KeyPropagation::Continue)
            .with_description("Save file")
            .with_source("user")
            .with_overlay_visibility(OverlayVisibility::Hidden);

        assert_eq!(opts.propagation(), KeyPropagation::Continue);
        assert_eq!(opts.description(), Some("Save file"));
        assert_eq!(opts.source().map(BindingSource::as_str), Some("user"));
        assert_eq!(opts.overlay_visibility(), OverlayVisibility::Hidden);
    }

    #[test]
    fn binding_options_can_track_binding_source() {
        let source = BindingSource::new("user");
        let options = BindingOptions::default().with_source(source.clone());

        assert_eq!(source.as_str(), "user");
        assert_eq!(options.source(), Some(&source));
    }

    #[test]
    fn binding_source_reserved_labels_are_case_insensitive() {
        assert_eq!(
            BindingSource::new("DEFAULT").precedence_tier(),
            BindingSourceTier::Default
        );
        assert_eq!(
            BindingSource::new("UsEr").precedence_tier(),
            BindingSourceTier::User
        );
        assert_eq!(
            BindingSource::new("plugin").precedence_tier(),
            BindingSourceTier::Standard
        );
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

    #[test]
    fn device_filter_name_contains_matches_substring() {
        let filter = DeviceFilter::name_contains("StreamDeck");
        let info = DeviceInfo::new("Elgato StreamDeck XL", 0x0fd9, 0x006c);
        assert!(filter.matches(&info));
    }

    #[test]
    fn device_filter_name_contains_rejects_non_matching() {
        let filter = DeviceFilter::name_contains("StreamDeck");
        let info = DeviceInfo::new("AT Translated Set 2 keyboard", 0x0001, 0x0001);
        assert!(!filter.matches(&info));
    }

    #[test]
    fn device_filter_usb_id_matches_exact() {
        let filter = DeviceFilter::usb(0x1234, 0x5678);
        let info = DeviceInfo::new("Some Device", 0x1234, 0x5678);
        assert!(filter.matches(&info));
    }

    #[test]
    fn device_filter_usb_id_rejects_wrong_vendor() {
        let filter = DeviceFilter::usb(0x1234, 0x5678);
        let info = DeviceInfo::new("Some Device", 0xFFFF, 0x5678);
        assert!(!filter.matches(&info));
    }

    #[test]
    fn device_filter_usb_id_rejects_wrong_product() {
        let filter = DeviceFilter::usb(0x1234, 0x5678);
        let info = DeviceInfo::new("Some Device", 0x1234, 0xFFFF);
        assert!(!filter.matches(&info));
    }

    #[test]
    fn binding_options_with_device() {
        let opts = BindingOptions::default().with_device(DeviceFilter::name_contains("StreamDeck"));
        assert!(opts.device().is_some());
        assert!(matches!(
            opts.device().unwrap(),
            DeviceFilter::NameContains(_)
        ));
    }

    #[test]
    fn binding_options_device_default_is_none() {
        let opts = BindingOptions::default();
        assert!(opts.device().is_none());
    }
}
