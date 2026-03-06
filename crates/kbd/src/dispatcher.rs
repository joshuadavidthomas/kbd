//! Binding dispatcher — finds which binding (if any) matches a key event.
//!
//! The [`Dispatcher`] walks the layer stack
//! top-down, checking bindings in each active layer, then global bindings.
//! Within each layer, speculative patterns (tap-hold, sequences) are checked
//! before immediate patterns (hotkeys).
//!
//! Returns a [`MatchResult`] — the matched
//! binding's action (or "no match" for forwarding).

mod layers;
mod query;
mod registry;
mod resolve;
mod sequence;
mod timeout;

use std::collections::HashMap;
use std::time::Instant;

use self::layers::LayerEffect;
use self::layers::LayerStackEntry;
use self::resolve::ScopeMatch;
use self::resolve::ScopeSequenceMatch;
use self::sequence::ActiveSequence;
use self::sequence::PendingStandalone;
use self::sequence::RegisteredSequenceBinding;
use self::sequence::SequenceBindingRef;
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
/// # fn main() -> Result<(), kbd::error::Error> {
/// let mut dispatcher = Dispatcher::new();
/// dispatcher.register(
///     Hotkey::new(Key::S).modifier(Modifier::Ctrl),
///     Action::Suppress,
/// )?;
///
/// let result = dispatcher.process(
///     &Hotkey::new(Key::S).modifier(Modifier::Ctrl),
///     KeyTransition::Press,
/// );
/// assert!(matches!(result, MatchResult::Matched { .. }));
/// # Ok(())
/// # }
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
/// # fn main() -> Result<(), kbd::error::Error> {
/// let mut dispatcher = Dispatcher::new();
///
/// // Define a navigation layer
/// let nav = Layer::new("nav")
///     .bind(Key::H, Action::Suppress)?
///     .bind(Key::J, Action::Suppress)?
///     .bind(Key::K, Action::Suppress)?
///     .bind(Key::L, Action::Suppress)?
///     .bind(Key::ESCAPE, Action::PopLayer)?
///     .swallow();
/// dispatcher.define_layer(nav)?;
///
/// // Activate the layer
/// dispatcher.push_layer("nav")?;
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
/// # Ok(())
/// # }
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

impl Dispatcher {
    /// Create a new empty dispatcher with no bindings or layers.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Return current in-progress sequence state, if any.
    #[must_use]
    pub fn pending_sequence(&self) -> Option<PendingSequenceInfo> {
        self.pending_sequence_snapshot()
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

            let scope_match = resolve::classify_layer_scope(stored, hotkey);
            let swallow_unmatched = matches!(stored.options.unmatched(), UnmatchedKeys::Swallow);

            match scope_match {
                ScopeMatch::SingleStepSequence { index } => {
                    let stored = &self.layers[&layer_name];
                    let sb = &stored.sequence_bindings[index];
                    let candidates = vec![SequenceStartCandidate::SingleStep {
                        binding_ref: MatchedBindingRef::SequenceLayer {
                            name: layer_name.clone(),
                            index,
                        },
                        layer_effect: LayerEffect::from_action(&sb.action),
                        propagation: sb.propagation,
                    }];
                    if let Some(outcome) =
                        self.start_sequences(candidates, now, next_priority, None)
                    {
                        return Some(outcome);
                    }
                }
                ScopeMatch::MultiStepSequences {
                    indices,
                    immediate_index,
                } => {
                    let stored = &self.layers[&layer_name];
                    let candidates: Vec<_> = indices
                        .iter()
                        .map(|&idx| {
                            let sb = &stored.sequence_bindings[idx];
                            SequenceStartCandidate::MultiStep {
                                binding_ref: SequenceBindingRef::Layer {
                                    name: layer_name.clone(),
                                    index: idx,
                                },
                                timeout: sb.options.timeout(),
                            }
                        })
                        .collect();
                    let pending_standalone = immediate_index.map(|idx| {
                        let lb = &stored.bindings[idx];
                        PendingStandalone {
                            binding_ref: MatchedBindingRef::Layer {
                                name: layer_name.clone(),
                                index: idx,
                            },
                            layer_effect: LayerEffect::from_action(&lb.action),
                            propagation: lb.propagation,
                        }
                    });
                    // `stored` is last used above; NLL releases the borrow.
                    if let Some(outcome) =
                        self.start_sequences(candidates, now, next_priority, pending_standalone)
                    {
                        return Some(outcome);
                    }
                }
                ScopeMatch::Immediate { index } => {
                    let stored = &self.layers[&layer_name];
                    let lb = &stored.bindings[index];
                    let propagation = lb.propagation;
                    let layer_effect = LayerEffect::from_action(&lb.action);
                    return Some(InternalOutcome::Matched {
                        layer_effect,
                        binding_ref: MatchedBindingRef::Layer {
                            name: layer_name,
                            index,
                        },
                        propagation,
                    });
                }
                ScopeMatch::None => {
                    if swallow_unmatched {
                        return Some(InternalOutcome::Suppressed);
                    }
                }
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
        let candidates: Vec<SequenceStartCandidate> = {
            let global_seqs = self.sorted_global_sequences();
            let scope_match =
                resolve::classify_scope_sequences(global_seqs.iter().map(|b| &b.sequence), hotkey);
            match scope_match {
                ScopeSequenceMatch::SingleStep { index } => {
                    let binding = global_seqs[index];
                    vec![SequenceStartCandidate::SingleStep {
                        binding_ref: MatchedBindingRef::SequenceGlobal(binding.id),
                        layer_effect: LayerEffect::from_action(&binding.action),
                        propagation: binding.propagation,
                    }]
                }
                ScopeSequenceMatch::MultiStep { indices } => indices
                    .iter()
                    .map(|&idx| {
                        let binding = global_seqs[idx];
                        SequenceStartCandidate::MultiStep {
                            binding_ref: SequenceBindingRef::Global(binding.id),
                            timeout: binding.options.timeout(),
                        }
                    })
                    .collect(),
                ScopeSequenceMatch::None => Vec::new(),
            }
        };
        // global_seqs dropped; self is unborrowed.

        if !candidates.is_empty() {
            let pending_standalone =
                self.pending_standalone_from_match(self.match_global_hotkey(hotkey));
            if let Some(outcome) =
                self.start_sequences(candidates, now, &mut next_priority, pending_standalone)
            {
                return outcome;
            }
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
            .define_layer(Layer::new("nav").bind(Key::H, Action::Suppress).unwrap())
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
            .define_layer(
                Layer::new("nav")
                    .bind(
                        Key::H,
                        Action::from(move || {
                            cc.fetch_add(1, Ordering::Relaxed);
                        }),
                    )
                    .unwrap(),
            )
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
