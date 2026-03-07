//! The unified binding types — pattern + action + options.
//!
//! A binding is the core unit: "when this input pattern matches, do this
//! action." [`BindingId`] uniquely identifies a binding. [`BindingOptions`] holds
//! per-binding configuration. [`RegisteredBinding`] pairs them with
//! a hotkey and action for engine storage.

use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::action::Action;
use crate::device::DeviceFilter;
use crate::hotkey::Hotkey;
use crate::policy::KeyPropagation;
use crate::policy::RateLimit;
use crate::policy::RepeatPolicy;

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
/// The dispatcher's registry recognizes two labels for precedence when
/// multiple global bindings share the same hotkey: `"default"` is
/// lower-priority and `"user"` is higher-priority (case-insensitive).
/// Other labels use the normal priority tier. See the registry module
/// for precedence resolution details.
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
/// use kbd::binding::{BindingOptions, BindingSource, OverlayVisibility};
/// use kbd::policy::KeyPropagation;
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
    /// Suppress rapid re-presses within this time window.
    ///
    /// When set, if the binding fires and the same hotkey is pressed
    /// again within the debounce window, the second press is throttled
    /// (consumed but action not executed).
    debounce: Option<Duration>,
    /// Cap how many times the action fires within a sliding time window.
    rate_limit: Option<RateLimit>,
    /// How OS auto-repeat events are handled for this binding.
    repeat_policy: RepeatPolicy,
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

    /// The debounce window for this binding, if set.
    ///
    /// When set, rapid re-presses within this duration are suppressed.
    #[must_use]
    pub const fn debounce(&self) -> Option<Duration> {
        self.debounce
    }

    /// Set a debounce window — suppress rapid re-presses within this
    /// duration.
    ///
    /// Debounce applies to press events only — it does not affect OS
    /// auto-repeat events (those are governed by
    /// [`repeat_policy`](Self::repeat_policy)).
    #[must_use]
    pub const fn with_debounce(mut self, window: Duration) -> Self {
        self.debounce = Some(window);
        self
    }

    /// The rate limit for this binding, if set.
    #[must_use]
    pub const fn rate_limit(&self) -> Option<RateLimit> {
        self.rate_limit
    }

    /// Set a rate limit — cap how many times the action fires within a
    /// sliding time window.
    #[must_use]
    pub const fn with_rate_limit(mut self, rate_limit: RateLimit) -> Self {
        self.rate_limit = Some(rate_limit);
        self
    }

    /// How OS auto-repeat events are handled for this binding.
    #[must_use]
    pub const fn repeat_policy(&self) -> RepeatPolicy {
        self.repeat_policy
    }

    /// Set the repeat policy for this binding.
    ///
    /// Controls whether OS auto-repeat events (from holding a key down)
    /// re-fire the binding's action.
    #[must_use]
    pub const fn with_repeat_policy(mut self, policy: RepeatPolicy) -> Self {
        self.repeat_policy = policy;
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
        use crate::policy::KeyPropagation;

        let opts = BindingOptions::default();
        assert_eq!(opts.propagation(), KeyPropagation::Stop);
        assert_eq!(opts.description(), None);
        assert_eq!(opts.source(), None);
        assert_eq!(opts.overlay_visibility(), OverlayVisibility::Visible);
    }

    #[test]
    fn binding_options_builder_chain() {
        use crate::policy::KeyPropagation;

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
    fn registered_binding_stores_fields() {
        use crate::policy::KeyPropagation;

        let id = BindingId::new();
        let hotkey = Hotkey::new(Key::S).modifier(Modifier::Ctrl);
        let binding = RegisteredBinding::new(id, hotkey.clone(), Action::Suppress);

        assert_eq!(binding.id(), id);
        assert_eq!(*binding.hotkey(), hotkey);
        assert_eq!(binding.propagation(), KeyPropagation::Stop);
    }

    #[test]
    fn registered_binding_with_propagation() {
        use crate::policy::KeyPropagation;

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

    #[test]
    fn repeat_policy_default_is_suppress() {
        let policy = RepeatPolicy::default();
        assert!(matches!(policy, RepeatPolicy::Suppress));
    }

    #[test]
    fn repeat_policy_custom_stores_delay_and_rate() {
        use crate::policy::RepeatTiming;

        let timing = RepeatTiming::new(Duration::from_millis(500), Duration::from_millis(30));
        let policy = RepeatPolicy::Custom(timing);
        match policy {
            RepeatPolicy::Custom(t) => {
                assert_eq!(t.delay(), Duration::from_millis(500));
                assert_eq!(t.rate(), Duration::from_millis(30));
            }
            _ => panic!("expected Custom variant"),
        }
    }

    #[test]
    fn rate_limit_construction() {
        let rl = RateLimit::new(3, Duration::from_secs(1));
        assert_eq!(rl.max_count(), 3);
        assert_eq!(rl.window(), Duration::from_secs(1));
    }

    #[test]
    fn binding_options_debounce_default_is_none() {
        let opts = BindingOptions::default();
        assert_eq!(opts.debounce(), None);
    }

    #[test]
    fn binding_options_with_debounce() {
        let opts = BindingOptions::default().with_debounce(Duration::from_millis(100));
        assert_eq!(opts.debounce(), Some(Duration::from_millis(100)));
    }

    #[test]
    fn binding_options_rate_limit_default_is_none() {
        let opts = BindingOptions::default();
        assert!(opts.rate_limit().is_none());
    }

    #[test]
    fn binding_options_with_rate_limit() {
        let rl = RateLimit::new(5, Duration::from_secs(1));
        let opts = BindingOptions::default().with_rate_limit(rl);
        let stored = opts.rate_limit().unwrap();
        assert_eq!(stored.max_count(), 5);
        assert_eq!(stored.window(), Duration::from_secs(1));
    }

    #[test]
    fn binding_options_repeat_policy_default_is_suppress() {
        let opts = BindingOptions::default();
        assert!(matches!(opts.repeat_policy(), RepeatPolicy::Suppress));
    }

    #[test]
    fn binding_options_with_repeat_policy() {
        let opts = BindingOptions::default().with_repeat_policy(RepeatPolicy::Allow);
        assert!(matches!(opts.repeat_policy(), RepeatPolicy::Allow));
    }

    #[test]
    fn binding_options_chains_all_new_options() {
        let opts = BindingOptions::default()
            .with_debounce(Duration::from_millis(50))
            .with_rate_limit(RateLimit::new(10, Duration::from_secs(1)))
            .with_repeat_policy(RepeatPolicy::Allow)
            .with_description("test");

        assert_eq!(opts.debounce(), Some(Duration::from_millis(50)));
        assert!(opts.rate_limit().is_some());
        assert!(matches!(opts.repeat_policy(), RepeatPolicy::Allow));
        assert_eq!(opts.description(), Some("test"));
    }
}
