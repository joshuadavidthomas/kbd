//! Binding dispatcher — finds which binding (if any) matches a key event.
//!
//! The [`Dispatcher`] walks the layer stack
//! top-down, checking bindings in each active layer, then global bindings.
//! Within each layer, speculative patterns (tap-hold, sequences) are checked
//! before immediate patterns (hotkeys).
//!
//! Returns a [`MatchResult`] — the matched
//! binding's action (or "no match" for forwarding).

mod query;

use std::collections::HashMap;
use std::time::Duration;
use std::time::Instant;

use crate::action::Action;
use crate::binding::BindingId;
use crate::binding::KeyPropagation;
use crate::binding::RegisteredBinding;
use crate::hotkey::Hotkey;
use crate::hotkey::Modifier;
use crate::key_state::KeyTransition;
use crate::layer::LayerName;
use crate::layer::StoredLayer;
use crate::layer::UnmatchedKeys;

/// Result of attempting to match a key event against registered bindings.
#[derive(Debug)]
#[non_exhaustive]
pub enum MatchResult<'a> {
    /// A binding matched. Contains the action and propagation setting.
    Matched {
        /// The action to execute.
        action: &'a Action,
        /// Whether to consume or forward the original key event.
        propagation: KeyPropagation,
    },
    /// No binding matched the event.
    NoMatch,
    /// The event was suppressed by a layer with `UnmatchedKeys::Swallow`.
    Suppressed,
    /// The event was not eligible for matching (modifier-only press, release, repeat).
    Ignored,
}

/// An entry in the layer stack, pairing the layer name with runtime state.
struct LayerStackEntry {
    name: LayerName,
    /// Remaining keypress count for oneshot layers. `None` means not oneshot.
    oneshot_remaining: Option<usize>,
    /// Timeout configuration and last activity timestamp.
    /// If set, the layer auto-pops when `Instant::now() - last_activity > timeout`.
    timeout: Option<LayerTimeout>,
}

struct LayerTimeout {
    duration: Duration,
    last_activity: Instant,
}

/// A synchronous hotkey matching engine.
///
/// `Dispatcher` is the embeddable engine. No threads, no channels, no evdev.
/// Consumers drive it from their own event loop — winit, GPUI, Smithay,
/// a game loop, whatever.
///
/// # Lifecycle
///
/// 1. Create with [`Dispatcher::new`]
/// 2. Register global bindings with [`register`](Dispatcher::register) or
///    [`register_binding`](Dispatcher::register_binding)
/// 3. Define layers with [`define_layer`](Dispatcher::define_layer), activate
///    with [`push_layer`](Dispatcher::push_layer)
/// 4. Feed key events via [`process`](Dispatcher::process) — returns a
///    [`MatchResult`] telling you what (if anything) matched
/// 5. Inspect state with [`list_bindings`](Dispatcher::list_bindings),
///    [`active_layers`](Dispatcher::active_layers), and
///    [`conflicts`](Dispatcher::conflicts)
///
/// # Examples
///
/// Register a global binding and match against it:
///
/// ```
/// use kbd::action::Action;
/// use kbd::dispatcher::{Dispatcher, MatchResult};
/// use kbd::hotkey::{Hotkey, Modifier};
/// use kbd::key::Key;
/// use kbd::key_state::KeyTransition;
///
/// let mut dispatcher = Dispatcher::new();
/// dispatcher.register(
///     Hotkey::new(Key::S).modifier(Modifier::Ctrl),
///     Action::Suppress,
/// ).unwrap();
///
/// let result = dispatcher.process(
///     &Hotkey::new(Key::S).modifier(Modifier::Ctrl),
///     KeyTransition::Press,
/// );
/// assert!(matches!(result, MatchResult::Matched { .. }));
/// ```
///
/// Using layers for modal editing:
///
/// ```
/// use kbd::action::Action;
/// use kbd::dispatcher::{Dispatcher, MatchResult};
/// use kbd::hotkey::{Hotkey, Modifier};
/// use kbd::key::Key;
/// use kbd::key_state::KeyTransition;
/// use kbd::layer::Layer;
///
/// let mut dispatcher = Dispatcher::new();
///
/// // Define a navigation layer
/// let nav = Layer::new("nav")
///     .bind(Key::H, Action::Suppress)
///     .bind(Key::J, Action::Suppress)
///     .bind(Key::K, Action::Suppress)
///     .bind(Key::L, Action::Suppress)
///     .bind(Key::ESCAPE, Action::PopLayer)
///     .swallow();
/// dispatcher.define_layer(nav).unwrap();
///
/// // Activate the layer
/// dispatcher.push_layer("nav").unwrap();
///
/// // H matches in the nav layer
/// let result = dispatcher.process(&Hotkey::new(Key::H), KeyTransition::Press);
/// assert!(matches!(result, MatchResult::Matched { .. }));
///
/// // Escape pops the layer via Action::PopLayer
/// dispatcher.process(&Hotkey::new(Key::ESCAPE), KeyTransition::Press);
///
/// // H no longer matches
/// let result = dispatcher.process(&Hotkey::new(Key::H), KeyTransition::Press);
/// assert!(matches!(result, MatchResult::NoMatch));
/// ```
#[derive(Default)]
pub struct Dispatcher {
    bindings_by_id: HashMap<BindingId, RegisteredBinding>,
    binding_ids_by_hotkey: HashMap<Hotkey, BindingId>,
    layers: HashMap<LayerName, StoredLayer>,
    layer_stack: Vec<LayerStackEntry>,
}

/// Internal reference to a matched binding, used to re-find the action
/// after layer mutations are applied.
enum MatchedBindingRef {
    Global(BindingId),
    Layer { name: LayerName, index: usize },
}

/// Internal match outcome that carries binding refs and layer effects.
enum InternalOutcome {
    Matched {
        binding_ref: MatchedBindingRef,
        layer_effect: LayerEffect,
        propagation: KeyPropagation,
    },
    Suppressed,
    NoMatch,
    Ignored,
}

/// Layer stack mutation extracted from a matched action.
enum LayerEffect {
    None,
    Push(LayerName),
    Pop,
    Toggle(LayerName),
}

impl LayerEffect {
    fn from_action(action: &Action) -> Self {
        match action {
            Action::PushLayer(name) => Self::Push(name.clone()),
            Action::PopLayer => Self::Pop,
            Action::ToggleLayer(name) => Self::Toggle(name.clone()),
            Action::Callback(_)
            | Action::EmitHotkey(..)
            | Action::EmitSequence(..)
            | Action::Suppress => Self::None,
        }
    }
}

impl Dispatcher {
    /// Create a new empty dispatcher with no bindings or layers.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a binding. Returns the assigned [`BindingId`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::AlreadyRegistered`](crate::error::Error::AlreadyRegistered)
    /// if a binding for the same hotkey exists.
    pub fn register(
        &mut self,
        hotkey: impl Into<Hotkey>,
        action: impl Into<Action>,
    ) -> Result<BindingId, crate::error::Error> {
        let id = BindingId::new();
        let binding = RegisteredBinding::new(id, hotkey.into(), action.into());
        self.register_binding(binding)?;
        Ok(id)
    }

    /// Register a [`RegisteredBinding`] with full options control.
    ///
    /// # Errors
    ///
    /// Returns [`Error::AlreadyRegistered`](crate::error::Error::AlreadyRegistered)
    /// if a binding for the same hotkey exists.
    pub fn register_binding(
        &mut self,
        binding: RegisteredBinding,
    ) -> Result<(), crate::error::Error> {
        let id = binding.id();
        let hotkey = binding.hotkey().clone();

        if self.bindings_by_id.contains_key(&id) || self.binding_ids_by_hotkey.contains_key(&hotkey)
        {
            return Err(crate::error::Error::AlreadyRegistered);
        }

        self.binding_ids_by_hotkey.insert(hotkey, id);
        self.bindings_by_id.insert(id, binding);
        Ok(())
    }

    /// Unregister a binding by its [`BindingId`].
    pub fn unregister(&mut self, id: BindingId) {
        if let Some(binding) = self.bindings_by_id.remove(&id) {
            self.binding_ids_by_hotkey.remove(binding.hotkey());
        }
    }

    /// Check whether a hotkey has a registered global binding.
    #[must_use]
    pub fn is_registered(&self, hotkey: &Hotkey) -> bool {
        self.binding_ids_by_hotkey.contains_key(hotkey)
    }

    /// Define a named layer. The layer is not active until pushed.
    ///
    /// # Errors
    ///
    /// Returns [`Error::LayerAlreadyDefined`](crate::error::Error::LayerAlreadyDefined)
    /// if a layer with the same name exists.
    pub fn define_layer(&mut self, layer: crate::layer::Layer) -> Result<(), crate::error::Error> {
        let (name, bindings, options) = layer.into_parts();
        match self.layers.entry(name) {
            std::collections::hash_map::Entry::Occupied(_) => {
                Err(crate::error::Error::LayerAlreadyDefined)
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(StoredLayer { bindings, options });
                Ok(())
            }
        }
    }

    /// Push a named layer onto the stack, activating its bindings.
    ///
    /// # Errors
    ///
    /// Returns [`Error::LayerNotDefined`](crate::error::Error::LayerNotDefined)
    /// if no layer with this name is defined.
    pub fn push_layer(&mut self, name: impl Into<LayerName>) -> Result<(), crate::error::Error> {
        let name = name.into();
        let stored = self
            .layers
            .get(&name)
            .ok_or(crate::error::Error::LayerNotDefined)?;
        let oneshot_remaining = stored.options.oneshot();
        let timeout = stored.options.timeout().map(|duration| LayerTimeout {
            duration,
            last_activity: Instant::now(),
        });
        self.layer_stack.push(LayerStackEntry {
            name,
            oneshot_remaining,
            timeout,
        });
        Ok(())
    }

    /// Pop the topmost layer from the stack.
    ///
    /// # Errors
    ///
    /// Returns [`Error::EmptyLayerStack`](crate::error::Error::EmptyLayerStack)
    /// if no layers are on the stack.
    pub fn pop_layer(&mut self) -> Result<LayerName, crate::error::Error> {
        self.layer_stack
            .pop()
            .map(|entry| entry.name)
            .ok_or(crate::error::Error::EmptyLayerStack)
    }

    /// Toggle a layer: push if not active, remove if active.
    ///
    /// # Errors
    ///
    /// Returns [`Error::LayerNotDefined`](crate::error::Error::LayerNotDefined)
    /// if no layer with this name is defined.
    pub fn toggle_layer(&mut self, name: impl Into<LayerName>) -> Result<(), crate::error::Error> {
        let name = name.into();
        if !self.layers.contains_key(&name) {
            return Err(crate::error::Error::LayerNotDefined);
        }
        if let Some(pos) = self
            .layer_stack
            .iter()
            .rposition(|entry| entry.name == name)
        {
            self.layer_stack.remove(pos);
        } else {
            self.push_layer(name)?;
        }
        Ok(())
    }

    /// Process a key event and return the match result.
    ///
    /// The caller provides the hotkey (key + currently active modifiers)
    /// and the key transition. The dispatcher walks the layer stack, finds
    /// the matching binding, and applies layer effects (push/pop/toggle)
    /// internally.
    ///
    /// Only key press events trigger matching — release and repeat events
    /// return `MatchResult::Ignored`. Modifier-only presses also return
    /// `MatchResult::Ignored`.
    pub fn process(&mut self, hotkey: &Hotkey, transition: KeyTransition) -> MatchResult<'_> {
        // Match and extract outcome (temporary borrow of &self)
        let outcome = self.match_extract(hotkey, transition);

        // Apply layer effects (&mut self)
        if let InternalOutcome::Matched {
            ref layer_effect, ..
        } = outcome
        {
            self.apply_layer_effect(layer_effect);
        }

        // Tick oneshot and reset timeouts for actionable events
        if !matches!(outcome, InternalOutcome::Ignored) {
            self.reset_layer_timeouts();
            // Skip oneshot tick for layer mutation events — the mutation
            // changes the stack, and the triggering keypress shouldn't
            // count against a newly-pushed oneshot layer.
            if !matches!(
                outcome,
                InternalOutcome::Matched {
                    layer_effect: LayerEffect::Push(_) | LayerEffect::Pop | LayerEffect::Toggle(_),
                    ..
                }
            ) {
                self.tick_oneshot_layers();
            }
        }

        // Convert to MatchResult by re-borrowing &self
        match outcome {
            InternalOutcome::Matched {
                binding_ref,
                propagation,
                ..
            } => {
                let action = self.resolve_binding(&binding_ref);
                MatchResult::Matched {
                    action,
                    propagation,
                }
            }
            InternalOutcome::Suppressed => MatchResult::Suppressed,
            InternalOutcome::NoMatch => MatchResult::NoMatch,
            InternalOutcome::Ignored => MatchResult::Ignored,
        }
    }

    /// Return the nearest layer timeout deadline, if any.
    ///
    /// Returns `None` if no timeout layers are active. The caller can
    /// use this to set poll timeouts in their event loop.
    #[must_use]
    pub fn next_timeout_deadline(&self) -> Option<Duration> {
        let now = Instant::now();
        let mut min_remaining = None;

        for entry in &self.layer_stack {
            if let Some(timeout) = &entry.timeout {
                let elapsed = now.duration_since(timeout.last_activity);
                let remaining = timeout.duration.saturating_sub(elapsed);
                min_remaining = Some(match min_remaining {
                    Some(current) => std::cmp::min(current, remaining),
                    None => remaining,
                });
            }
        }

        min_remaining
    }

    /// Check all active timeout layers and pop any that have expired.
    ///
    /// Call this periodically from your event loop (e.g., on each poll
    /// cycle) to ensure timeout layers auto-pop on schedule.
    pub fn check_timeouts(&mut self) {
        let now = Instant::now();
        self.layer_stack.retain(|entry| {
            if let Some(timeout) = &entry.timeout {
                now.duration_since(timeout.last_activity) < timeout.duration
            } else {
                true
            }
        });
    }

    /// Match a hotkey against the layer stack and global bindings.
    /// Returns an `InternalOutcome` that carries binding refs and layer effects.
    fn match_extract(&self, hotkey: &Hotkey, transition: KeyTransition) -> InternalOutcome {
        // Only match on key press events
        if !matches!(transition, KeyTransition::Press) {
            return InternalOutcome::Ignored;
        }

        // Modifier-only presses don't trigger hotkeys
        if Modifier::from_key(hotkey.key()).is_some() {
            return InternalOutcome::Ignored;
        }

        // Walk layer stack top-down
        for entry in self.layer_stack.iter().rev() {
            if let Some(stored) = self.layers.get(&entry.name) {
                for (index, layer_binding) in stored.bindings.iter().enumerate() {
                    if layer_binding.hotkey == *hotkey {
                        return InternalOutcome::Matched {
                            binding_ref: MatchedBindingRef::Layer {
                                name: entry.name.clone(),
                                index,
                            },
                            layer_effect: LayerEffect::from_action(&layer_binding.action),
                            propagation: layer_binding.propagation,
                        };
                    }
                }

                if matches!(stored.options.unmatched(), UnmatchedKeys::Swallow) {
                    return InternalOutcome::Suppressed;
                }
            }
        }

        // Fall through to global bindings
        if let Some(&id) = self.binding_ids_by_hotkey.get(hotkey)
            && let Some(binding) = self.bindings_by_id.get(&id)
        {
            return InternalOutcome::Matched {
                binding_ref: MatchedBindingRef::Global(id),
                layer_effect: LayerEffect::from_action(binding.action()),
                propagation: binding.propagation(),
            };
        }

        InternalOutcome::NoMatch
    }

    /// Resolve a binding reference back to its action.
    fn resolve_binding(&self, binding_ref: &MatchedBindingRef) -> &Action {
        match binding_ref {
            MatchedBindingRef::Global(id) => self.bindings_by_id[id].action(),
            MatchedBindingRef::Layer { name, index } => &self.layers[name].bindings[*index].action,
        }
    }

    /// Apply a layer effect extracted from a matched action.
    fn apply_layer_effect(&mut self, effect: &LayerEffect) {
        match effect {
            LayerEffect::None => {}
            LayerEffect::Push(name) => {
                let _ = self.push_layer(name.clone());
            }
            LayerEffect::Pop => {
                let _ = self.pop_layer();
            }
            LayerEffect::Toggle(name) => {
                let _ = self.toggle_layer(name.clone());
            }
        }
    }

    /// Reset timeout clocks on all active timeout layers (activity occurred).
    fn reset_layer_timeouts(&mut self) {
        let now = Instant::now();
        for entry in &mut self.layer_stack {
            if let Some(timeout) = &mut entry.timeout {
                timeout.last_activity = now;
            }
        }
    }

    /// Decrement oneshot counters and pop expired layers.
    fn tick_oneshot_layers(&mut self) {
        let mut pop_index = None;
        for (i, entry) in self.layer_stack.iter_mut().enumerate().rev() {
            if let Some(remaining) = &mut entry.oneshot_remaining {
                *remaining = remaining.saturating_sub(1);
                if *remaining == 0 {
                    pop_index = Some(i);
                }
                break;
            }
        }
        if let Some(index) = pop_index {
            self.layer_stack.remove(index);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use super::*;
    use crate::key::Key;
    use crate::layer::Layer;

    #[test]
    fn register_returns_unique_id() {
        let mut dispatcher = Dispatcher::new();
        let id1 = dispatcher
            .register(Hotkey::new(Key::A), Action::Suppress)
            .unwrap();
        let id2 = dispatcher
            .register(Hotkey::new(Key::B), Action::Suppress)
            .unwrap();
        assert_ne!(id1, id2);
    }

    #[test]
    fn is_registered_reflects_state() {
        let mut dispatcher = Dispatcher::new();
        let hotkey = Hotkey::new(Key::C).modifier(Modifier::Ctrl);
        assert!(!dispatcher.is_registered(&hotkey));

        dispatcher
            .register(hotkey.clone(), Action::Suppress)
            .unwrap();
        assert!(dispatcher.is_registered(&hotkey));
    }

    #[test]
    fn process_requires_exact_modifiers() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register(
                Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                Action::Suppress,
            )
            .unwrap();

        // Missing modifier
        let result = dispatcher.process(&Hotkey::new(Key::C), KeyTransition::Press);
        assert!(matches!(result, MatchResult::NoMatch));

        // Extra modifier
        let result = dispatcher.process(
            &Hotkey::new(Key::C)
                .modifier(Modifier::Ctrl)
                .modifier(Modifier::Shift),
            KeyTransition::Press,
        );
        assert!(matches!(result, MatchResult::NoMatch));
    }

    #[test]
    fn process_ignores_modifier_only_presses() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register(Hotkey::new(Key::CONTROL_LEFT), Action::Suppress)
            .unwrap();

        let result = dispatcher.process(&Hotkey::new(Key::CONTROL_LEFT), KeyTransition::Press);
        assert!(matches!(result, MatchResult::Ignored));
    }

    #[test]
    fn unmatched_key_falls_through_to_global() {
        let mut dispatcher = Dispatcher::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        dispatcher
            .register(Hotkey::new(Key::X), move || {
                cc.fetch_add(1, Ordering::Relaxed);
            })
            .unwrap();
        dispatcher
            .define_layer(Layer::new("nav").bind(Key::H, Action::Suppress))
            .unwrap();
        dispatcher.push_layer("nav").unwrap();

        let result = dispatcher.process(&Hotkey::new(Key::X), KeyTransition::Press);
        if let MatchResult::Matched {
            action: Action::Callback(cb),
            ..
        } = result
        {
            cb();
        }
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn process_applies_push_layer_action() {
        let mut dispatcher = Dispatcher::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        dispatcher
            .define_layer(Layer::new("nav").bind(
                Key::H,
                Action::from(move || {
                    cc.fetch_add(1, Ordering::Relaxed);
                }),
            ))
            .unwrap();
        dispatcher
            .register(
                Hotkey::new(Key::F1),
                Action::PushLayer(LayerName::from("nav")),
            )
            .unwrap();

        // Press F1 → pushes nav layer
        dispatcher.process(&Hotkey::new(Key::F1), KeyTransition::Press);

        // Now H should match in nav layer
        let result = dispatcher.process(&Hotkey::new(Key::H), KeyTransition::Press);
        if let MatchResult::Matched {
            action: Action::Callback(cb),
            ..
        } = result
        {
            cb();
        }
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn oneshot_layer_pushed_via_action_not_immediately_decremented() {
        let mut dispatcher = Dispatcher::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        let layer = Layer::new("oneshot")
            .bind(
                Key::H,
                Action::from(move || {
                    cc.fetch_add(1, Ordering::Relaxed);
                }),
            )
            .oneshot(1);
        dispatcher.define_layer(layer).unwrap();

        // Register a global binding that pushes the oneshot layer
        dispatcher
            .register(
                Hotkey::new(Key::F1),
                Action::PushLayer(LayerName::from("oneshot")),
            )
            .unwrap();

        // Press F1 → pushes oneshot layer (should NOT consume a oneshot count)
        dispatcher.process(&Hotkey::new(Key::F1), KeyTransition::Press);

        // First keypress in the oneshot layer — should match and then pop
        let result = dispatcher.process(&Hotkey::new(Key::H), KeyTransition::Press);
        if let MatchResult::Matched {
            action: Action::Callback(cb),
            ..
        } = result
        {
            cb();
        }
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        // Second press → layer should be gone now
        let result = dispatcher.process(&Hotkey::new(Key::H), KeyTransition::Press);
        assert!(matches!(result, MatchResult::NoMatch));
    }
}
