//! [`Layer`] — a named collection of bindings, stackable.
//!
//! Layers are the organizational unit. When active, a layer's bindings
//! participate in matching. Layers stack: most recently activated is
//! checked first. Global bindings act as an always-active base layer.
//!
//! Layer is a builder — construct with `Layer::new("name")`, add bindings
//! with `.bind()`, configure with `.oneshot()` / `.swallow()` / `.timeout()`,
//! then hand to `manager.define_layer(layer)`.

use std::time::Duration;

use crate::action::Action;
use crate::action::LayerName;
use crate::binding::KeyPropagation;
use crate::key::Hotkey;

/// Whether unmatched keys in an active layer fall through to lower layers.
///
/// # Examples
///
/// ```
/// use kbd::action::Action;
/// use kbd::key::Key;
/// use kbd::layer::{Layer, UnmatchedKeys};
///
/// // A navigation layer that only captures H/J/K/L.
/// // Other keys (like Ctrl+S) still reach global bindings.
/// let nav = Layer::new("nav")
///     .bind(Key::H, Action::Suppress)
///     .bind(Key::J, Action::Suppress);
/// assert_eq!(nav.options().unmatched(), UnmatchedKeys::Fallthrough);
///
/// // A modal layer that captures ALL keys — nothing falls through.
/// // Useful for insert-mode or game-input modes.
/// let modal = Layer::new("modal")
///     .bind(Key::H, Action::Suppress)
///     .swallow();
/// assert_eq!(modal.options().unmatched(), UnmatchedKeys::Swallow);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum UnmatchedKeys {
    /// Unmatched keys pass to the next layer down the stack.
    #[default]
    Fallthrough,
    /// Unmatched keys are consumed (swallowed) by this layer.
    Swallow,
}

/// Per-layer behavioral options.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LayerOptions {
    /// If set, automatically pop the layer after this many keypresses.
    oneshot: Option<usize>,
    /// Whether unmatched keys are consumed or fall through.
    unmatched: UnmatchedKeys,
    /// If set, automatically pop the layer after this duration of inactivity.
    timeout: Option<Duration>,
    /// Human-readable label for this layer, used for overlay grouping.
    description: Option<Box<str>>,
}

impl LayerOptions {
    /// If set, automatically pop the layer after this many keypresses.
    #[must_use]
    pub const fn oneshot(&self) -> Option<usize> {
        self.oneshot
    }

    /// Whether unmatched keys are consumed or fall through.
    #[must_use]
    pub const fn unmatched(&self) -> UnmatchedKeys {
        self.unmatched
    }

    /// If set, automatically pop the layer after this duration of inactivity.
    #[must_use]
    pub const fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    /// Human-readable label for this layer, used for overlay grouping.
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Set unmatched key behavior.
    #[must_use]
    pub const fn with_unmatched(mut self, behavior: UnmatchedKeys) -> Self {
        self.unmatched = behavior;
        self
    }
}

/// A single binding within a layer.
#[derive(Debug)]
pub(crate) struct LayerBinding {
    pub(crate) hotkey: Hotkey,
    pub(crate) action: Action,
    pub(crate) propagation: KeyPropagation,
}

/// Engine-internal representation of a stored layer definition.
pub(crate) struct StoredLayer {
    pub(crate) bindings: Vec<LayerBinding>,
    pub(crate) options: LayerOptions,
}

impl std::fmt::Debug for StoredLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoredLayer")
            .field("bindings", &self.bindings.len())
            .field("options", &self.options)
            .finish()
    }
}

/// A named collection of bindings that can be activated and deactivated.
///
/// Construct via the builder pattern, then register with
/// [`Dispatcher::define_layer`](crate::Dispatcher::define_layer).
///
/// # Examples
///
/// Basic layer with vim-style navigation:
///
/// ```
/// use kbd::action::Action;
/// use kbd::key::Key;
/// use kbd::layer::Layer;
///
/// let nav = Layer::new("nav")
///     .bind(Key::H, Action::Suppress)
///     .bind(Key::J, Action::Suppress)
///     .bind(Key::K, Action::Suppress)
///     .bind(Key::L, Action::Suppress)
///     .description("Vim navigation keys")
///     .swallow();
///
/// assert_eq!(nav.name().as_str(), "nav");
/// assert_eq!(nav.binding_count(), 4);
/// ```
///
/// Oneshot layer that auto-pops after one keypress:
///
/// ```
/// use kbd::action::Action;
/// use kbd::key::Key;
/// use kbd::layer::Layer;
///
/// let leader = Layer::new("leader")
///     .bind(Key::F, Action::Suppress)
///     .bind(Key::B, Action::Suppress)
///     .oneshot(1);
/// ```
///
/// Layer with a timeout that auto-pops after inactivity:
///
/// ```
/// use std::time::Duration;
/// use kbd::action::Action;
/// use kbd::key::Key;
/// use kbd::layer::Layer;
///
/// let timed = Layer::new("quick-nav")
///     .bind(Key::N, Action::Suppress)
///     .bind(Key::P, Action::Suppress)
///     .timeout(Duration::from_secs(2));
/// ```
pub struct Layer {
    name: LayerName,
    bindings: Vec<LayerBinding>,
    options: LayerOptions,
}

impl Layer {
    /// Create a new layer with the given name.
    #[must_use]
    pub fn new(name: impl Into<LayerName>) -> Self {
        Self {
            name: name.into(),
            bindings: Vec::new(),
            options: LayerOptions::default(),
        }
    }

    /// Add a binding to this layer.
    #[must_use]
    pub fn bind(mut self, hotkey: impl Into<Hotkey>, action: impl Into<Action>) -> Self {
        self.bindings.push(LayerBinding {
            hotkey: hotkey.into(),
            action: action.into(),
            propagation: KeyPropagation::default(),
        });
        self
    }

    /// Set the layer to swallow unmatched keys (consume instead of fallthrough).
    #[must_use]
    pub fn swallow(mut self) -> Self {
        self.options.unmatched = UnmatchedKeys::Swallow;
        self
    }

    /// Set the layer to auto-pop after `depth` keypresses (oneshot mode).
    #[must_use]
    pub fn oneshot(mut self, depth: usize) -> Self {
        self.options.oneshot = Some(depth);
        self
    }

    /// Set the layer to auto-pop after `duration` of inactivity.
    #[must_use]
    pub fn timeout(mut self, duration: Duration) -> Self {
        self.options.timeout = Some(duration);
        self
    }

    /// Set a human-readable description for this layer.
    ///
    /// Used for overlay grouping and help screen display.
    #[must_use]
    pub fn description(mut self, description: impl Into<Box<str>>) -> Self {
        self.options.description = Some(description.into());
        self
    }

    /// The layer's name.
    #[must_use]
    pub fn name(&self) -> &LayerName {
        &self.name
    }

    /// The layer's options.
    #[must_use]
    pub fn options(&self) -> &LayerOptions {
        &self.options
    }

    /// The number of bindings in this layer.
    #[must_use]
    pub fn binding_count(&self) -> usize {
        self.bindings.len()
    }

    /// Consume this layer and return its constituent parts.
    #[must_use]
    pub(crate) fn into_parts(self) -> (LayerName, Vec<LayerBinding>, LayerOptions) {
        (self.name, self.bindings, self.options)
    }
}

impl std::fmt::Debug for Layer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Layer")
            .field("name", &self.name)
            .field("bindings", &self.bindings.len())
            .field("options", &self.options)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::action::Action;
    use crate::key::Key;
    use crate::key::Modifier;

    #[test]
    fn layer_new_creates_with_name() {
        let layer = Layer::new("nav");
        assert_eq!(layer.name().as_str(), "nav");
    }

    #[test]
    fn layer_new_has_empty_bindings() {
        let layer = Layer::new("test");
        assert_eq!(layer.binding_count(), 0);
    }

    #[test]
    fn layer_new_has_default_options() {
        let layer = Layer::new("test");
        assert_eq!(*layer.options(), LayerOptions::default());
    }

    #[test]
    fn layer_bind_adds_binding() {
        let layer = Layer::new("nav").bind(Key::H, Action::Suppress);
        assert_eq!(layer.binding_count(), 1);
    }

    #[test]
    fn layer_bind_multiple_bindings() {
        let layer = Layer::new("nav")
            .bind(Key::H, Action::Suppress)
            .bind(Key::J, Action::Suppress)
            .bind(Key::K, Action::Suppress)
            .bind(Key::L, Action::Suppress);
        assert_eq!(layer.binding_count(), 4);
    }

    #[test]
    fn layer_bind_preserves_hotkey() {
        let layer = Layer::new("nav").bind(
            Hotkey::new(Key::H).modifier(Modifier::Ctrl),
            Action::Suppress,
        );
        let (_, bindings, _) = layer.into_parts();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].hotkey.key(), Key::H);
        assert_eq!(bindings[0].hotkey.modifiers(), &[Modifier::Ctrl]);
    }

    #[test]
    fn layer_bind_accepts_closure() {
        let layer = Layer::new("test").bind(Key::A, || println!("fired"));
        assert_eq!(layer.binding_count(), 1);
    }

    #[test]
    fn layer_swallow_sets_option() {
        let layer = Layer::new("test").swallow();
        assert_eq!(layer.options().unmatched(), UnmatchedKeys::Swallow);
    }

    #[test]
    fn layer_oneshot_sets_depth() {
        let layer = Layer::new("test").oneshot(3);
        assert_eq!(layer.options().oneshot(), Some(3));
    }

    #[test]
    fn layer_timeout_sets_duration() {
        let duration = Duration::from_secs(5);
        let layer = Layer::new("test").timeout(duration);
        assert_eq!(layer.options().timeout(), Some(duration));
    }

    #[test]
    fn layer_builder_chains_all_options() {
        let layer = Layer::new("nav")
            .bind(Key::H, Action::Suppress)
            .bind(Key::J, Action::Suppress)
            .description("Navigation keys")
            .swallow()
            .oneshot(1)
            .timeout(Duration::from_millis(500));

        assert_eq!(layer.name().as_str(), "nav");
        assert_eq!(layer.binding_count(), 2);
        assert_eq!(layer.options().description(), Some("Navigation keys"));
        assert_eq!(layer.options().unmatched(), UnmatchedKeys::Swallow);
        assert_eq!(layer.options().oneshot(), Some(1));
        assert_eq!(layer.options().timeout(), Some(Duration::from_millis(500)));
    }

    #[test]
    fn layer_options_default_is_fallthrough_no_oneshot_no_timeout_no_description() {
        let options = LayerOptions::default();
        assert_eq!(options.oneshot(), None);
        assert_eq!(options.unmatched(), UnmatchedKeys::Fallthrough);
        assert_eq!(options.timeout(), None);
        assert_eq!(options.description(), None);
    }

    #[test]
    fn layer_name_from_string() {
        let layer = Layer::new(String::from("dynamic"));
        assert_eq!(layer.name().as_str(), "dynamic");
    }

    #[test]
    fn layer_into_parts_decomposes() {
        let layer = Layer::new("nav").bind(Key::H, Action::Suppress).swallow();

        let (name, bindings, options) = layer.into_parts();
        assert_eq!(name.as_str(), "nav");
        assert_eq!(bindings.len(), 1);
        assert_eq!(options.unmatched(), UnmatchedKeys::Swallow);
    }

    #[test]
    fn layer_description_sets_label() {
        let layer = Layer::new("nav").description("Navigation keys");
        assert_eq!(layer.options().description(), Some("Navigation keys"));
    }

    #[test]
    fn layer_description_preserved_in_into_parts() {
        let layer = Layer::new("nav")
            .bind(Key::H, Action::Suppress)
            .description("Navigation keys");

        let (_, _, options) = layer.into_parts();
        assert_eq!(options.description(), Some("Navigation keys"));
    }
}
