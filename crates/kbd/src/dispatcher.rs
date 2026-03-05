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
mod sequence;
mod timeout;

use std::collections::HashMap;
use std::time::Duration;
use std::time::Instant;

use self::sequence::ActiveSequence;
use self::sequence::PendingStandalone;
use self::sequence::RegisteredSequenceBinding;
use self::sequence::SequenceBindingRef;
use self::sequence::SequencePrefixKind;
use self::sequence::SequenceStartCandidate;
use crate::action::Action;
use crate::binding::BindingId;
use crate::binding::KeyPropagation;
use crate::binding::RegisteredBinding;
use crate::hotkey::Hotkey;
use crate::hotkey::HotkeySequence;
use crate::hotkey::Modifier;
use crate::key_state::KeyTransition;
use crate::layer::LayerName;
use crate::layer::StoredLayer;
use crate::layer::UnmatchedKeys;
use crate::sequence::PendingSequenceInfo;
use crate::sequence::SequenceInput;
use crate::sequence::SequenceOptions;

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
    /// Sequence is in progress and waiting for more steps.
    Pending {
        /// Number of steps already matched.
        steps_matched: usize,
        /// Number of steps still required.
        steps_remaining: usize,
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
    sequence_bindings_by_id: HashMap<BindingId, RegisteredSequenceBinding>,
    sequence_ids_by_value: HashMap<HotkeySequence, BindingId>,
    layers: HashMap<LayerName, StoredLayer>,
    layer_stack: Vec<LayerStackEntry>,
    active_sequences: Vec<ActiveSequence>,
    pending_standalone: Option<PendingStandalone>,
}

/// Internal reference to a matched binding, used to re-find the action
/// after layer mutations are applied.
#[derive(Clone)]
enum MatchedBindingRef {
    Global(BindingId),
    Layer { name: LayerName, index: usize },
    SequenceGlobal(BindingId),
    SequenceLayer { name: LayerName, index: usize },
}

/// Internal match outcome that carries binding refs and layer effects.
enum InternalOutcome {
    Matched {
        binding_ref: MatchedBindingRef,
        layer_effect: LayerEffect,
        propagation: KeyPropagation,
    },
    Pending {
        steps_matched: usize,
        steps_remaining: usize,
    },
    Suppressed,
    NoMatch,
    Ignored,
}

/// Layer stack mutation extracted from a matched action.
#[derive(Clone)]
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

    /// Register a multi-step sequence binding with default sequence options.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Parse`](crate::error::Error::Parse) when sequence input
    /// conversion fails, or
    /// [`Error::AlreadyRegistered`](crate::error::Error::AlreadyRegistered)
    /// if a binding for the same sequence already exists.
    pub fn register_sequence(
        &mut self,
        sequence: impl SequenceInput,
        action: impl Into<Action>,
    ) -> Result<BindingId, crate::error::Error> {
        self.register_sequence_with_options(sequence, action, SequenceOptions::default())
    }

    /// Register a sequence with explicit sequence options.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Parse`](crate::error::Error::Parse) when sequence input
    /// conversion fails, or
    /// [`Error::AlreadyRegistered`](crate::error::Error::AlreadyRegistered)
    /// if a binding for the same sequence already exists.
    pub fn register_sequence_with_options(
        &mut self,
        sequence: impl SequenceInput,
        action: impl Into<Action>,
        options: SequenceOptions,
    ) -> Result<BindingId, crate::error::Error> {
        let id = BindingId::new();
        let sequence = sequence.into_sequence()?;
        self.register_sequence_binding_with_id(id, sequence, action.into(), options)?;
        Ok(id)
    }

    /// Register a sequence binding with a caller-provided binding ID.
    ///
    /// # Errors
    ///
    /// Returns [`Error::AlreadyRegistered`](crate::error::Error::AlreadyRegistered)
    /// if a binding for the same sequence already exists.
    pub(crate) fn register_sequence_binding_with_id(
        &mut self,
        id: BindingId,
        sequence: HotkeySequence,
        action: Action,
        options: SequenceOptions,
    ) -> Result<(), crate::error::Error> {
        let binding = RegisteredSequenceBinding::new(id, sequence, action, options);
        self.register_sequence_binding(binding)
    }

    /// Return current in-progress sequence state, if any.
    #[must_use]
    pub fn pending_sequence(&self) -> Option<PendingSequenceInfo> {
        self.pending_sequence_snapshot()
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

        if self.bindings_by_id.contains_key(&id)
            || self.sequence_bindings_by_id.contains_key(&id)
            || self.binding_ids_by_hotkey.contains_key(&hotkey)
        {
            return Err(crate::error::Error::AlreadyRegistered);
        }

        self.binding_ids_by_hotkey.insert(hotkey, id);
        self.bindings_by_id.insert(id, binding);
        Ok(())
    }

    fn register_sequence_binding(
        &mut self,
        binding: RegisteredSequenceBinding,
    ) -> Result<(), crate::error::Error> {
        let id = binding.id;
        let sequence = binding.sequence.clone();

        if self.sequence_bindings_by_id.contains_key(&id)
            || self.bindings_by_id.contains_key(&id)
            || self.sequence_ids_by_value.contains_key(&sequence)
        {
            return Err(crate::error::Error::AlreadyRegistered);
        }

        self.sequence_ids_by_value.insert(sequence, id);
        self.sequence_bindings_by_id.insert(id, binding);
        Ok(())
    }

    /// Unregister a binding by its [`BindingId`].
    pub fn unregister(&mut self, id: BindingId) {
        if let Some(binding) = self.bindings_by_id.remove(&id) {
            self.binding_ids_by_hotkey.remove(binding.hotkey());
        }

        if let Some(binding) = self.sequence_bindings_by_id.remove(&id) {
            self.sequence_ids_by_value.remove(&binding.sequence);
        }

        self.active_sequences
            .retain(|active| !matches!(active.binding_ref, SequenceBindingRef::Global(global_id) if global_id == id));

        if self.pending_standalone.as_ref().is_some_and(|pending| {
            matches!(
                pending.binding_ref,
                MatchedBindingRef::Global(global_id) | MatchedBindingRef::SequenceGlobal(global_id)
                    if global_id == id
            )
        }) {
            self.pending_standalone = None;
        }

        if self.active_sequences.is_empty() {
            self.pending_standalone = None;
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
        let (name, bindings, sequence_bindings, options) = layer.into_parts();
        match self.layers.entry(name) {
            std::collections::hash_map::Entry::Occupied(_) => {
                Err(crate::error::Error::LayerAlreadyDefined)
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(StoredLayer {
                    bindings,
                    sequence_bindings,
                    options,
                });
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
        let name = self
            .layer_stack
            .pop()
            .map(|entry| entry.name)
            .ok_or(crate::error::Error::EmptyLayerStack)?;
        self.clear_sequences_for_layer_if_inactive(&name);
        Ok(name)
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
            let removed = self.layer_stack.remove(pos);
            self.clear_sequences_for_layer_if_inactive(&removed.name);
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
        let outcome = self.match_extract(hotkey, transition);

        if let InternalOutcome::Matched {
            ref layer_effect, ..
        } = outcome
        {
            self.apply_layer_effect(layer_effect);
        }

        if !matches!(outcome, InternalOutcome::Ignored) {
            self.reset_layer_timeouts();
            if !matches!(
                outcome,
                InternalOutcome::Matched {
                    layer_effect: LayerEffect::Push(_) | LayerEffect::Pop | LayerEffect::Toggle(_),
                    ..
                } | InternalOutcome::Pending { .. }
            ) {
                self.tick_oneshot_layers();
            }
        }

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
            InternalOutcome::Pending {
                steps_matched,
                steps_remaining,
            } => MatchResult::Pending {
                steps_matched,
                steps_remaining,
            },
            InternalOutcome::Suppressed => MatchResult::Suppressed,
            InternalOutcome::NoMatch => MatchResult::NoMatch,
            InternalOutcome::Ignored => MatchResult::Ignored,
        }
    }

    fn match_extract(&mut self, hotkey: &Hotkey, transition: KeyTransition) -> InternalOutcome {
        if !matches!(transition, KeyTransition::Press) {
            return InternalOutcome::Ignored;
        }

        if Modifier::from_key(hotkey.key()).is_some() {
            return InternalOutcome::Ignored;
        }

        if let Some(outcome) = self.match_active_sequences(hotkey) {
            return outcome;
        }

        let now = Instant::now();
        let mut next_priority = 0usize;

        if let Some(outcome) = self.match_layers(hotkey, now, &mut next_priority) {
            return outcome;
        }

        self.match_globals(hotkey, now, next_priority)
    }

    fn match_layers(
        &mut self,
        hotkey: &Hotkey,
        now: Instant,
        next_priority: &mut usize,
    ) -> Option<InternalOutcome> {
        let layer_names: Vec<_> = self
            .layer_stack
            .iter()
            .rev()
            .map(|entry| entry.name.clone())
            .collect();

        for layer_name in layer_names {
            let Some(stored) = self.layers.get(&layer_name) else {
                continue;
            };

            let mut candidates = Vec::new();
            for (index, sequence_binding) in stored.sequence_bindings.iter().enumerate() {
                match sequence::classify_sequence_prefix(&sequence_binding.sequence, hotkey) {
                    SequencePrefixKind::None => {}
                    SequencePrefixKind::SingleStep => {
                        candidates.push(SequenceStartCandidate::SingleStep {
                            binding_ref: MatchedBindingRef::SequenceLayer {
                                name: layer_name.clone(),
                                index,
                            },
                            layer_effect: LayerEffect::from_action(&sequence_binding.action),
                            propagation: sequence_binding.propagation,
                        });
                    }
                    SequencePrefixKind::MultiStep => {
                        candidates.push(SequenceStartCandidate::MultiStep {
                            binding_ref: SequenceBindingRef::Layer {
                                name: layer_name.clone(),
                                index,
                            },
                            timeout: sequence_binding.options.timeout(),
                        });
                    }
                }
            }
            let swallow_unmatched = matches!(stored.options.unmatched(), UnmatchedKeys::Swallow);

            let pending_standalone =
                self.pending_standalone_from_match(self.match_layer_hotkey(&layer_name, hotkey));
            if let Some(outcome) =
                self.start_sequences(candidates, now, next_priority, pending_standalone)
            {
                return Some(outcome);
            }

            if let Some((binding_ref, propagation)) = self.match_layer_hotkey(&layer_name, hotkey) {
                return Some(InternalOutcome::Matched {
                    layer_effect: LayerEffect::from_action(self.resolve_binding(&binding_ref)),
                    binding_ref,
                    propagation,
                });
            }

            if swallow_unmatched {
                return Some(InternalOutcome::Suppressed);
            }
        }

        None
    }

    fn match_globals(
        &mut self,
        hotkey: &Hotkey,
        now: Instant,
        mut next_priority: usize,
    ) -> InternalOutcome {
        let mut global_sequences: Vec<_> = self
            .sequence_bindings_by_id
            .values()
            .filter(|binding| {
                !matches!(
                    sequence::classify_sequence_prefix(&binding.sequence, hotkey),
                    SequencePrefixKind::None
                )
            })
            .collect();
        global_sequences.sort_by_key(|binding| binding.id.as_u64());

        let mut candidates = Vec::new();
        for binding in global_sequences {
            match sequence::classify_sequence_prefix(&binding.sequence, hotkey) {
                SequencePrefixKind::None => {}
                SequencePrefixKind::SingleStep => {
                    candidates.push(SequenceStartCandidate::SingleStep {
                        binding_ref: MatchedBindingRef::SequenceGlobal(binding.id),
                        layer_effect: LayerEffect::from_action(&binding.action),
                        propagation: binding.propagation,
                    });
                }
                SequencePrefixKind::MultiStep => {
                    candidates.push(SequenceStartCandidate::MultiStep {
                        binding_ref: SequenceBindingRef::Global(binding.id),
                        timeout: binding.options.timeout(),
                    });
                }
            }
        }

        let pending_standalone =
            self.pending_standalone_from_match(self.match_global_hotkey(hotkey));
        if let Some(outcome) =
            self.start_sequences(candidates, now, &mut next_priority, pending_standalone)
        {
            return outcome;
        }

        if let Some((binding_ref, propagation)) = self.match_global_hotkey(hotkey) {
            return InternalOutcome::Matched {
                layer_effect: LayerEffect::from_action(self.resolve_binding(&binding_ref)),
                binding_ref,
                propagation,
            };
        }

        InternalOutcome::NoMatch
    }

    fn match_layer_hotkey(
        &self,
        layer_name: &LayerName,
        hotkey: &Hotkey,
    ) -> Option<(MatchedBindingRef, KeyPropagation)> {
        let stored = self.layers.get(layer_name)?;
        stored
            .bindings
            .iter()
            .enumerate()
            .find_map(|(index, layer_binding)| {
                if layer_binding.hotkey == *hotkey {
                    Some((
                        MatchedBindingRef::Layer {
                            name: layer_name.clone(),
                            index,
                        },
                        layer_binding.propagation,
                    ))
                } else {
                    None
                }
            })
    }

    fn match_global_hotkey(&self, hotkey: &Hotkey) -> Option<(MatchedBindingRef, KeyPropagation)> {
        let id = *self.binding_ids_by_hotkey.get(hotkey)?;
        let binding = self.bindings_by_id.get(&id)?;
        Some((MatchedBindingRef::Global(id), binding.propagation()))
    }

    /// Resolve a binding reference back to its action.
    fn resolve_binding(&self, binding_ref: &MatchedBindingRef) -> &Action {
        match binding_ref {
            MatchedBindingRef::Global(id) => self.bindings_by_id[id].action(),
            MatchedBindingRef::Layer { name, index } => &self.layers[name].bindings[*index].action,
            MatchedBindingRef::SequenceGlobal(id) => &self.sequence_bindings_by_id[id].action,
            MatchedBindingRef::SequenceLayer { name, index } => {
                &self.layers[name].sequence_bindings[*index].action
            }
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
}
