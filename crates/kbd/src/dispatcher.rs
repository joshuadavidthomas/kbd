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
mod tap_hold;
mod throttle;
mod timeout;

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::time::Instant;

use self::layers::LayerEffect;
use self::layers::LayerStackEntry;
use self::resolve::LayerMatch;
use self::resolve::SequencePrefixMatch;
use self::sequence::ActiveSequence;
use self::sequence::PendingStandalone;
use self::sequence::SequenceBindingRef;
use self::sequence::SequenceStartCandidate;
use self::sequence::StandaloneMatch;
use self::tap_hold::TapHoldBinding;
use self::tap_hold::TapHoldOutcome;
use self::tap_hold::TapHoldState;
use self::throttle::ThrottleTracker;
pub use self::timeout::PendingTimeout;
use crate::action::Action;
use crate::binding::BindingId;
use crate::binding::RegisteredBinding;
use crate::binding::RegisteredSequenceBinding;
use crate::device::DeviceContext;
use crate::hotkey::Hotkey;
use crate::hotkey::HotkeySequence;
use crate::hotkey::Modifier;
use crate::key::Key;
use crate::key_state::KeyTransition;
use crate::layer::LayerName;
use crate::layer::StoredLayer;
use crate::layer::UnmatchedKeys;
use crate::policy::KeyPropagation;
use crate::policy::RepeatPolicy;
use crate::sequence::PendingSequenceInfo;
use crate::tap_hold::TapHoldOptions;

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
        /// How OS auto-repeat events should be handled for this binding.
        repeat_policy: RepeatPolicy,
    },
    /// A binding matched but was throttled (debounce or rate limit).
    ///
    /// The action should NOT execute. Key forwarding still respects the
    /// binding's `propagation` setting — a `Continue` binding forwards the
    /// event even when throttled. Distinct from [`Suppressed`](Self::Suppressed),
    /// which means a layer swallowed an unmatched key.
    Throttled {
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
/// 2. Register global bindings with [`register`](Dispatcher::register),
///    [`register_with_options`](Dispatcher::register_with_options), or
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
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut dispatcher = Dispatcher::new();
/// dispatcher.register(
///     Hotkey::new(Key::S).modifier(Modifier::Ctrl),
///     Action::Suppress,
/// )?;
///
/// let result = dispatcher.process(
///     Hotkey::new(Key::S).modifier(Modifier::Ctrl),
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
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
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
/// let result = dispatcher.process(Hotkey::new(Key::H), KeyTransition::Press);
/// assert!(matches!(result, MatchResult::Matched { .. }));
///
/// // Escape pops the layer via Action::PopLayer
/// dispatcher.process(Hotkey::new(Key::ESCAPE), KeyTransition::Press);
///
/// // H no longer matches
/// let result = dispatcher.process(Hotkey::new(Key::H), KeyTransition::Press);
/// assert!(matches!(result, MatchResult::NoMatch));
/// # Ok(())
/// # }
/// ```
#[derive(Default)]
pub struct Dispatcher {
    bindings_by_id: HashMap<BindingId, RegisteredBinding>,
    binding_ids_by_hotkey: HashMap<Hotkey, Vec<BindingId>>,
    sequence_bindings_by_id: BTreeMap<BindingId, RegisteredSequenceBinding>,
    sequence_ids_by_value: HashMap<HotkeySequence, BindingId>,
    layers: HashMap<LayerName, StoredLayer>,
    layer_stack: Vec<LayerStackEntry>,
    active_sequences: Vec<ActiveSequence>,
    pending_standalone: Option<PendingStandalone>,
    throttle_tracker: ThrottleTracker,
    tap_hold: TapHoldState,
}

/// Internal reference to a matched binding, used to re-find the action
/// after layer mutations are applied.
#[derive(Clone)]
pub(crate) enum MatchedBindingRef {
    Global(BindingId),
    Layer {
        id: BindingId,
        name: LayerName,
        index: usize,
    },
    SequenceGlobal(BindingId),
    SequenceLayer {
        id: BindingId,
        name: LayerName,
        index: usize,
    },
}

impl MatchedBindingRef {
    /// The unique binding ID regardless of where the binding lives.
    pub(crate) fn binding_id(&self) -> BindingId {
        match self {
            Self::Global(id)
            | Self::Layer { id, .. }
            | Self::SequenceGlobal(id)
            | Self::SequenceLayer { id, .. } => *id,
        }
    }
}

/// Internal match outcome that carries binding refs and layer effects.
enum BindingMatch {
    Matched {
        binding_ref: MatchedBindingRef,
        layer_effect: LayerEffect,
        propagation: KeyPropagation,
        repeat_policy: RepeatPolicy,
    },
    Throttled {
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

    /// Register a tap-hold binding.
    ///
    /// The `tap_action` fires when the key is pressed and released quickly
    /// (before the threshold). The `hold_action` fires when the key is held
    /// past the threshold or interrupted by another keypress.
    ///
    /// # Errors
    ///
    /// Returns an error if a tap-hold binding is already registered for the
    /// same key.
    pub fn register_tap_hold(
        &mut self,
        key: Key,
        tap_action: impl Into<Action>,
        hold_action: impl Into<Action>,
        options: TapHoldOptions,
    ) -> Result<BindingId, crate::error::RegisterError> {
        if self.tap_hold.is_registered(key) {
            return Err(crate::error::RegisterError::AlreadyRegistered);
        }
        let id = BindingId::new();
        self.tap_hold.register(TapHoldBinding {
            id,
            key,
            tap_action: tap_action.into(),
            hold_action: hold_action.into(),
            options,
        });
        Ok(id)
    }

    /// Check if a key has a tap-hold binding registered.
    ///
    /// Used by the engine to route release events for tap-hold keys
    /// through the dispatcher (for tap resolution) instead of the press
    /// cache.
    #[must_use]
    pub fn is_tap_hold_key(&self, key: Key) -> bool {
        self.tap_hold.is_registered(key)
    }

    /// Define a named layer. The layer is not active until pushed.
    ///
    /// # Errors
    ///
    /// Returns [`LayerError::AlreadyDefined`](crate::error::LayerError::AlreadyDefined)
    /// if a layer with the same name exists.
    pub fn define_layer(
        &mut self,
        layer: crate::layer::Layer,
    ) -> Result<(), crate::error::LayerError> {
        let (name, bindings, sequence_bindings, options) = layer.into_parts();
        match self.layers.entry(name) {
            std::collections::hash_map::Entry::Occupied(_) => {
                Err(crate::error::LayerError::AlreadyDefined)
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
    pub fn process(&mut self, hotkey: Hotkey, transition: KeyTransition) -> MatchResult<'_> {
        self.process_internal(hotkey, transition, None)
    }

    /// Process a key event with device context.
    ///
    /// Like [`process`](Self::process), but also carries device identity
    /// and per-device modifier state. This enables:
    ///
    /// - **Device-specific bindings**: bindings with a
    ///   [`DeviceFilter`](crate::device::DeviceFilter) only match events
    ///   from matching devices.
    /// - **Per-device modifier isolation**: device-filtered bindings use
    ///   the modifiers from [`DeviceContext::device_modifiers`] instead of
    ///   the aggregate modifiers in the `hotkey` argument.
    ///
    /// Global bindings (no device filter) are unaffected — they match
    /// against the aggregate hotkey as usual.
    pub fn process_with_device(
        &mut self,
        hotkey: Hotkey,
        transition: KeyTransition,
        device: &DeviceContext<'_>,
    ) -> MatchResult<'_> {
        self.process_internal(hotkey, transition, Some(device))
    }

    fn process_internal(
        &mut self,
        hotkey: Hotkey,
        transition: KeyTransition,
        device: Option<&DeviceContext<'_>>,
    ) -> MatchResult<'_> {
        // Fast path: non-Press events (Release, Repeat) with no active
        // tap-hold state are always Ignored. This skips tap-hold processing,
        // match_binding, throttle checks, and the final outcome match —
        // keeping the common case for release/repeat events near-zero cost.
        if !matches!(transition, KeyTransition::Press) && !self.tap_hold.has_state() {
            return MatchResult::Ignored;
        }

        // Tap-hold is checked first — it intercepts events before normal
        // matching, similar to how speculative patterns (sequences) take
        // priority over immediate patterns (hotkeys).
        let tap_hold_outcome = self.process_tap_hold(hotkey.key(), transition);

        match tap_hold_outcome {
            TapHoldOutcome::Consumed | TapHoldOutcome::RepeatConsumed => {
                return MatchResult::Matched {
                    action: &Action::Suppress,
                    propagation: KeyPropagation::Stop,
                    repeat_policy: RepeatPolicy::Suppress,
                };
            }
            TapHoldOutcome::TapResolved { binding_id } => {
                return match self.tap_hold.tap_action(binding_id) {
                    Some(action) => MatchResult::Matched {
                        action,
                        propagation: KeyPropagation::Stop,
                        repeat_policy: RepeatPolicy::Suppress,
                    },
                    None => MatchResult::Ignored,
                };
            }
            TapHoldOutcome::PassThrough => {
                // Fall through to normal matching.
            }
        }

        let outcome = self.match_binding(hotkey, transition, device);

        // Check debounce/rate-limit for matched bindings.
        // Throttled matches do NOT apply layer effects — if a PushLayer
        // action is throttled, the layer is not pushed.
        let outcome = self.check_throttle(outcome);

        if let BindingMatch::Matched {
            ref layer_effect, ..
        } = outcome
        {
            self.apply_layer_effect(layer_effect);
        }

        if !matches!(outcome, BindingMatch::Ignored) {
            // Reset layer inactivity timeouts so layers remain alive
            // while the user is actively typing.
            let now = Instant::now();
            for entry in &mut self.layer_stack {
                if let Some(timeout) = &mut entry.timeout {
                    timeout.last_activity = now;
                }
            }
            if !matches!(
                outcome,
                BindingMatch::Matched {
                    layer_effect: LayerEffect::Push(_) | LayerEffect::Pop | LayerEffect::Toggle(_),
                    ..
                } | BindingMatch::Pending { .. }
                    | BindingMatch::Throttled { .. }
            ) {
                // Tick the topmost oneshot layer, popping it when its
                // count reaches zero. Only one oneshot layer is ticked
                // per event.
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
                    let removed = self.layer_stack.remove(index);
                    self.clear_sequences_for_layer_if_inactive(&removed.name);
                }
            }
        }

        match outcome {
            BindingMatch::Matched {
                binding_ref,
                propagation,
                repeat_policy,
                ..
            } => {
                let action = self.resolve_binding(&binding_ref);
                MatchResult::Matched {
                    action,
                    propagation,
                    repeat_policy,
                }
            }
            BindingMatch::Throttled { propagation } => MatchResult::Throttled { propagation },
            BindingMatch::Pending {
                steps_matched,
                steps_remaining,
            } => MatchResult::Pending {
                steps_matched,
                steps_remaining,
            },
            BindingMatch::Suppressed => MatchResult::Suppressed,
            BindingMatch::NoMatch => MatchResult::NoMatch,
            BindingMatch::Ignored => MatchResult::Ignored,
        }
    }

    /// Process tap-hold events, returning the outcome directly.
    ///
    /// This is a thin wrapper: fast-path check, timestamp, dispatch to
    /// `on_press`/`on_release`/`on_repeat`. No callback execution happens
    /// here — hold actions resolved by interrupt are buffered in
    /// `TapHoldState` and drained through the `pending_timeouts` pipeline,
    /// where the engine handles them identically to timeout-resolved holds.
    fn process_tap_hold(&mut self, key: Key, transition: KeyTransition) -> TapHoldOutcome {
        // Fast path: skip all tap-hold work when no bindings are registered
        // and no keys are actively being tracked. This keeps the common case
        // (no tap-hold configured) essentially zero-cost.
        if !self.tap_hold.has_state() {
            return TapHoldOutcome::PassThrough;
        }

        match transition {
            KeyTransition::Press => self.tap_hold.on_press(key, Instant::now()),
            KeyTransition::Release => self.tap_hold.on_release(key),
            KeyTransition::Repeat => self.tap_hold.on_repeat(key),
            // KeyTransition is #[non_exhaustive]; future variants pass through.
            #[allow(unreachable_patterns)]
            _ => TapHoldOutcome::PassThrough,
        }
    }

    fn match_binding(
        &mut self,
        hotkey: Hotkey,
        transition: KeyTransition,
        device: Option<&DeviceContext<'_>>,
    ) -> BindingMatch {
        if !matches!(transition, KeyTransition::Press) {
            return BindingMatch::Ignored;
        }

        if Modifier::from_key(hotkey.key()).is_some() {
            return BindingMatch::Ignored;
        }

        if let Some(outcome) = self.match_active_sequences(hotkey) {
            return outcome;
        }

        let now = Instant::now();
        let mut next_priority = 0usize;

        if let Some(outcome) = self.match_layers(hotkey, now, &mut next_priority, device) {
            return outcome;
        }

        self.match_globals(hotkey, now, next_priority, device)
    }

    fn match_layers(
        &mut self,
        hotkey: Hotkey,
        now: Instant,
        next_priority: &mut usize,
        device: Option<&DeviceContext<'_>>,
    ) -> Option<BindingMatch> {
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

            let layer_match = resolve::classify_layer(stored, hotkey, device);
            let swallow_unmatched = matches!(stored.options.unmatched(), UnmatchedKeys::Swallow);

            match layer_match {
                LayerMatch::SingleStepSequence { index } => {
                    let stored = &self.layers[&layer_name];
                    let sb = &stored.sequence_bindings[index];
                    let candidates = vec![SequenceStartCandidate::SingleStep {
                        binding_ref: MatchedBindingRef::SequenceLayer {
                            id: sb.id,
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
                LayerMatch::MultiStepSequences {
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
                                    id: sb.id,
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
                            inner: StandaloneMatch {
                                binding_ref: MatchedBindingRef::Layer {
                                    id: lb.id(),
                                    name: layer_name.clone(),
                                    index: idx,
                                },
                                propagation: lb.options().propagation(),
                                repeat_policy: lb.options().repeat_policy(),
                            },
                            layer_effect: LayerEffect::from_action(lb.action()),
                        }
                    });
                    // `stored` is last used above; NLL releases the borrow.
                    if let Some(outcome) =
                        self.start_sequences(candidates, now, next_priority, pending_standalone)
                    {
                        return Some(outcome);
                    }
                }
                LayerMatch::Immediate { index } => {
                    let stored = &self.layers[&layer_name];
                    let lb = &stored.bindings[index];
                    let id = lb.id();
                    let propagation = lb.options().propagation();
                    let repeat_policy = lb.options().repeat_policy();
                    let layer_effect = LayerEffect::from_action(lb.action());
                    return Some(BindingMatch::Matched {
                        layer_effect,
                        binding_ref: MatchedBindingRef::Layer {
                            id,
                            name: layer_name,
                            index,
                        },
                        propagation,
                        repeat_policy,
                    });
                }
                LayerMatch::None => {
                    if swallow_unmatched {
                        return Some(BindingMatch::Suppressed);
                    }
                }
            }
        }

        None
    }

    fn match_globals(
        &mut self,
        hotkey: Hotkey,
        now: Instant,
        mut next_priority: usize,
        device: Option<&DeviceContext<'_>>,
    ) -> BindingMatch {
        let candidates: Vec<SequenceStartCandidate> = {
            let global_seqs: Vec<_> = self.sequence_bindings_by_id.values().collect();
            let prefix_match = resolve::classify_sequence_prefixes(
                global_seqs.iter().map(|b| &b.sequence),
                hotkey,
            );
            match prefix_match {
                SequencePrefixMatch::SingleStep { index } => {
                    let binding = global_seqs[index];
                    vec![SequenceStartCandidate::SingleStep {
                        binding_ref: MatchedBindingRef::SequenceGlobal(binding.id),
                        layer_effect: LayerEffect::from_action(&binding.action),
                        propagation: binding.propagation,
                    }]
                }
                SequencePrefixMatch::MultiStep { indices } => indices
                    .iter()
                    .map(|&idx| {
                        let binding = global_seqs[idx];
                        SequenceStartCandidate::MultiStep {
                            binding_ref: SequenceBindingRef::Global(binding.id),
                            timeout: binding.options.timeout(),
                        }
                    })
                    .collect(),
                SequencePrefixMatch::None => Vec::new(),
            }
        };
        // global_seqs dropped; self is unborrowed.

        if !candidates.is_empty() {
            let pending_standalone =
                self.pending_standalone_from_match(self.match_global_hotkey(hotkey, device));
            if let Some(outcome) =
                self.start_sequences(candidates, now, &mut next_priority, pending_standalone)
            {
                return outcome;
            }
        }

        if let Some((binding_ref, propagation, repeat_policy)) =
            self.match_global_hotkey(hotkey, device)
        {
            return BindingMatch::Matched {
                layer_effect: LayerEffect::from_action(self.resolve_binding(&binding_ref)),
                binding_ref,
                propagation,
                repeat_policy,
            };
        }

        BindingMatch::NoMatch
    }

    /// Return the highest-precedence non-device-filtered binding ID for a hotkey.
    ///
    /// Used exclusively by the introspection/query path (not the runtime
    /// matching path). Device-filtered bindings are skipped because without
    /// a [`DeviceContext`] we cannot determine whether they would fire.
    fn active_global_binding_id(&self, hotkey: Hotkey) -> Option<BindingId> {
        self.binding_ids_by_hotkey.get(&hotkey).and_then(|ids| {
            ids.iter()
                .rev()
                .find(|id| {
                    self.bindings_by_id
                        .get(id)
                        .is_some_and(|b| b.options().device().is_none())
                })
                .copied()
        })
    }

    fn match_global_hotkey(
        &self,
        hotkey: Hotkey,
        device: Option<&DeviceContext<'_>>,
    ) -> Option<(MatchedBindingRef, KeyPropagation, RepeatPolicy)> {
        // First, try device-filtered bindings if we have device context.
        // These use per-device modifier isolation.
        if let Some(ctx) = device {
            if let Some(result) = self.match_device_filtered_global(hotkey, ctx) {
                return Some(result);
            }
        }

        // Fall through to non-device-filtered bindings (aggregate modifiers).
        // Walk from highest precedence to lowest, skipping device-filtered
        // bindings — they were already checked above with modifier isolation.
        let ids = self.binding_ids_by_hotkey.get(&hotkey)?;
        for id in ids.iter().rev() {
            if let Some(binding) = self.bindings_by_id.get(id) {
                if binding.options().device().is_none() {
                    return Some((
                        MatchedBindingRef::Global(*id),
                        binding.propagation(),
                        binding.options().repeat_policy(),
                    ));
                }
            }
        }
        None
    }

    /// Match device-filtered global bindings using per-device modifier isolation.
    fn match_device_filtered_global(
        &self,
        hotkey: Hotkey,
        device: &DeviceContext<'_>,
    ) -> Option<(MatchedBindingRef, KeyPropagation, RepeatPolicy)> {
        // Build the device-specific candidate hotkey for modifier isolation.
        // Hotkey is Copy, so no allocation needed.
        let lookup_key = if let Some(device_mods) = device.device_modifiers() {
            Hotkey::with_modifiers(hotkey.key(), device_mods)
        } else {
            hotkey
        };

        // Look up bindings registered for the device-specific hotkey.
        // Walk from highest precedence to lowest for deterministic ordering.
        let ids = self.binding_ids_by_hotkey.get(&lookup_key)?;
        for id in ids.iter().rev() {
            if let Some(binding) = self.bindings_by_id.get(id) {
                if let Some(filter) = binding.options().device() {
                    if filter.matches(device.info()) {
                        return Some((
                            MatchedBindingRef::Global(*id),
                            binding.propagation(),
                            binding.options().repeat_policy(),
                        ));
                    }
                }
            }
        }

        None
    }

    /// Resolve a binding reference back to its action.
    fn resolve_binding(&self, binding_ref: &MatchedBindingRef) -> &Action {
        match binding_ref {
            MatchedBindingRef::Global(id) => self.bindings_by_id[id].action(),
            MatchedBindingRef::Layer { name, index, .. } => {
                self.layers[name].bindings[*index].action()
            }
            MatchedBindingRef::SequenceGlobal(id) => &self.sequence_bindings_by_id[id].action,
            MatchedBindingRef::SequenceLayer { name, index, .. } => {
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
    use std::time::Duration;

    use super::*;
    use crate::binding::BindingOptions;
    use crate::device::DeviceFilter;
    use crate::device::DeviceInfo;
    use crate::hotkey::ModifierSet;
    use crate::key::Key;
    use crate::layer::Layer;
    use crate::policy::RateLimit;
    use crate::tap_hold::TapHoldOptions;

    #[test]
    fn device_binding_matches_on_correct_device() {
        let mut dispatcher = Dispatcher::new();
        let device_a = DeviceInfo::new("StreamDeck XL", 0x0fd9, 0x006c);
        let device_b = DeviceInfo::new("AT Translated Set 2 keyboard", 0x0001, 0x0001);

        dispatcher
            .register_with_options(
                Hotkey::new(Key::A),
                Action::Suppress,
                BindingOptions::default().with_device(DeviceFilter::name_contains("StreamDeck")),
            )
            .unwrap();

        let ctx_a = DeviceContext::new(10, &device_a);
        let result =
            dispatcher.process_with_device(Hotkey::new(Key::A), KeyTransition::Press, &ctx_a);
        assert!(matches!(result, MatchResult::Matched { .. }));

        let ctx_b = DeviceContext::new(11, &device_b);
        let result =
            dispatcher.process_with_device(Hotkey::new(Key::A), KeyTransition::Press, &ctx_b);
        assert!(matches!(result, MatchResult::NoMatch));
    }

    #[test]
    fn device_binding_misses_on_wrong_device() {
        let mut dispatcher = Dispatcher::new();
        let wrong_device = DeviceInfo::new("Regular Keyboard", 0x0001, 0x0001);

        dispatcher
            .register_with_options(
                Hotkey::new(Key::A),
                Action::Suppress,
                BindingOptions::default().with_device(DeviceFilter::usb(0x0fd9, 0x006c)),
            )
            .unwrap();

        let ctx = DeviceContext::new(10, &wrong_device);
        let result =
            dispatcher.process_with_device(Hotkey::new(Key::A), KeyTransition::Press, &ctx);
        assert!(matches!(result, MatchResult::NoMatch));
    }

    #[test]
    fn per_device_modifier_isolation() {
        let mut dispatcher = Dispatcher::new();
        let device_b = DeviceInfo::new("StreamDeck", 0x0fd9, 0x006c);

        dispatcher
            .register_with_options(
                Hotkey::new(Key::A).modifier(Modifier::Ctrl),
                Action::Suppress,
                BindingOptions::default().with_device(DeviceFilter::name_contains("StreamDeck")),
            )
            .unwrap();

        let ctx_b = DeviceContext::new(11, &device_b).with_device_modifiers(ModifierSet::NONE);

        let candidate = Hotkey::new(Key::A).modifier(Modifier::Ctrl);
        let result = dispatcher.process_with_device(candidate, KeyTransition::Press, &ctx_b);
        assert!(matches!(result, MatchResult::NoMatch));
    }

    #[test]
    fn global_binding_uses_aggregate_modifiers() {
        let mut dispatcher = Dispatcher::new();
        let device_b = DeviceInfo::new("Regular Keyboard", 0x0001, 0x0001);

        dispatcher
            .register(
                Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                Action::Suppress,
            )
            .unwrap();

        let ctx = DeviceContext::new(11, &device_b);
        let candidate = Hotkey::new(Key::C).modifier(Modifier::Ctrl);
        let result = dispatcher.process_with_device(candidate, KeyTransition::Press, &ctx);
        assert!(matches!(result, MatchResult::Matched { .. }));
    }

    #[test]
    fn device_filtered_binding_uses_device_modifiers_only() {
        let mut dispatcher = Dispatcher::new();
        let streamdeck = DeviceInfo::new("StreamDeck", 0x0fd9, 0x006c);

        dispatcher
            .register_with_options(
                Hotkey::new(Key::A).modifier(Modifier::Ctrl),
                Action::Suppress,
                BindingOptions::default().with_device(DeviceFilter::name_contains("StreamDeck")),
            )
            .unwrap();

        let ctx = DeviceContext::new(10, &streamdeck).with_device_modifiers(ModifierSet::CTRL);

        let candidate = Hotkey::new(Key::A).modifier(Modifier::Ctrl);
        let result = dispatcher.process_with_device(candidate, KeyTransition::Press, &ctx);
        assert!(matches!(result, MatchResult::Matched { .. }));
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
        let result = dispatcher.process(Hotkey::new(Key::C), KeyTransition::Press);
        assert!(matches!(result, MatchResult::NoMatch));

        // Extra modifier
        let result = dispatcher.process(
            Hotkey::new(Key::C)
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

        let result = dispatcher.process(Hotkey::new(Key::CONTROL_LEFT), KeyTransition::Press);
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

        let result = dispatcher.process(Hotkey::new(Key::X), KeyTransition::Press);
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
        dispatcher.process(Hotkey::new(Key::F1), KeyTransition::Press);

        // Now H should match in nav layer
        let result = dispatcher.process(Hotkey::new(Key::H), KeyTransition::Press);
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
    fn device_and_global_bindings_coexist_for_same_hotkey() {
        let mut dispatcher = Dispatcher::new();

        // Register a device-filtered binding and a global binding for the same key
        dispatcher
            .register_with_options(
                Hotkey::new(Key::A),
                Action::Suppress,
                BindingOptions::default().with_device(DeviceFilter::name_contains("StreamDeck")),
            )
            .unwrap();

        dispatcher
            .register(Hotkey::new(Key::A), Action::Suppress)
            .unwrap();
    }

    #[test]
    fn global_binding_falls_through_when_device_filter_misses() {
        let mut dispatcher = Dispatcher::new();
        let global_counter = Arc::new(AtomicUsize::new(0));
        let gc = Arc::clone(&global_counter);

        let streamdeck = DeviceInfo::new("StreamDeck XL", 0x0fd9, 0x006c);
        let keyboard = DeviceInfo::new("AT Translated Set 2 keyboard", 0x0001, 0x0001);

        // Higher-tier device-filtered binding
        dispatcher
            .register_with_options(
                Hotkey::new(Key::A),
                Action::Suppress,
                BindingOptions::default()
                    .with_device(DeviceFilter::name_contains("StreamDeck"))
                    .with_source("user"),
            )
            .unwrap();

        // Lower-tier global binding
        dispatcher
            .register_with_options(
                Hotkey::new(Key::A),
                Action::from(move || {
                    gc.fetch_add(1, Ordering::Relaxed);
                }),
                BindingOptions::default().with_source("default"),
            )
            .unwrap();

        // From the StreamDeck: device-filtered binding should match
        let ctx_sd = DeviceContext::new(10, &streamdeck).with_device_modifiers(ModifierSet::NONE);
        let result =
            dispatcher.process_with_device(Hotkey::new(Key::A), KeyTransition::Press, &ctx_sd);
        assert!(matches!(result, MatchResult::Matched { .. }));

        // From the keyboard: device filter doesn't match, should fall through to global
        let ctx_kb = DeviceContext::new(11, &keyboard).with_device_modifiers(ModifierSet::NONE);
        let result =
            dispatcher.process_with_device(Hotkey::new(Key::A), KeyTransition::Press, &ctx_kb);
        if let MatchResult::Matched {
            action: Action::Callback(cb),
            ..
        } = result
        {
            cb();
        }
        assert_eq!(global_counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn multiple_device_filters_same_hotkey_same_tier() {
        let mut dispatcher = Dispatcher::new();

        // Two device-filtered bindings for the same hotkey, different filters
        dispatcher
            .register_with_options(
                Hotkey::new(Key::A),
                Action::Suppress,
                BindingOptions::default().with_device(DeviceFilter::name_contains("StreamDeck")),
            )
            .unwrap();

        dispatcher
            .register_with_options(
                Hotkey::new(Key::A),
                Action::Suppress,
                BindingOptions::default().with_device(DeviceFilter::usb(0x1234, 0x5678)),
            )
            .unwrap();

        // Same device filter for same hotkey should still be rejected
        let result = dispatcher.register_with_options(
            Hotkey::new(Key::A),
            Action::Suppress,
            BindingOptions::default().with_device(DeviceFilter::name_contains("StreamDeck")),
        );
        assert!(matches!(
            result,
            Err(crate::error::RegisterError::AlreadyRegistered)
        ));
    }

    #[test]
    fn process_without_device_context_skips_device_filtered_bindings() {
        let mut dispatcher = Dispatcher::new();
        let global_counter = Arc::new(AtomicUsize::new(0));
        let gc = Arc::clone(&global_counter);

        // Device-filtered binding
        dispatcher
            .register_with_options(
                Hotkey::new(Key::A),
                Action::Suppress,
                BindingOptions::default().with_device(DeviceFilter::name_contains("StreamDeck")),
            )
            .unwrap();

        // Global binding
        dispatcher
            .register_with_options(
                Hotkey::new(Key::A),
                Action::from(move || {
                    gc.fetch_add(1, Ordering::Relaxed);
                }),
                BindingOptions::default(),
            )
            .unwrap();

        // Using process() without device context should skip device-filtered
        // and match the global binding
        let result = dispatcher.process(Hotkey::new(Key::A), KeyTransition::Press);
        if let MatchResult::Matched {
            action: Action::Callback(cb),
            ..
        } = result
        {
            cb();
        }
        assert_eq!(global_counter.load(Ordering::Relaxed), 1);
    }

    // Debounce tests

    #[test]
    fn debounce_first_press_matches() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register_with_options(
                Hotkey::new(Key::A),
                Action::Suppress,
                BindingOptions::default().with_debounce(Duration::from_millis(100)),
            )
            .unwrap();

        let result = dispatcher.process(Hotkey::new(Key::A), KeyTransition::Press);
        assert!(matches!(result, MatchResult::Matched { .. }));
    }

    #[test]
    fn debounce_rapid_repress_is_throttled() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register_with_options(
                Hotkey::new(Key::A),
                Action::Suppress,
                BindingOptions::default().with_debounce(Duration::from_millis(100)),
            )
            .unwrap();

        let result = dispatcher.process(Hotkey::new(Key::A), KeyTransition::Press);
        assert!(matches!(result, MatchResult::Matched { .. }));

        let result = dispatcher.process(Hotkey::new(Key::A), KeyTransition::Press);
        assert!(matches!(result, MatchResult::Throttled { .. }));
    }

    #[test]
    fn debounce_after_window_expires_matches_again() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register_with_options(
                Hotkey::new(Key::A),
                Action::Suppress,
                BindingOptions::default().with_debounce(Duration::from_millis(10)),
            )
            .unwrap();

        let result = dispatcher.process(Hotkey::new(Key::A), KeyTransition::Press);
        assert!(matches!(result, MatchResult::Matched { .. }));

        std::thread::sleep(Duration::from_millis(15));

        let result = dispatcher.process(Hotkey::new(Key::A), KeyTransition::Press);
        assert!(matches!(result, MatchResult::Matched { .. }));
    }

    #[test]
    fn debounce_is_per_binding() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register_with_options(
                Hotkey::new(Key::A),
                Action::Suppress,
                BindingOptions::default().with_debounce(Duration::from_millis(100)),
            )
            .unwrap();
        dispatcher
            .register(Hotkey::new(Key::B), Action::Suppress)
            .unwrap();

        let result = dispatcher.process(Hotkey::new(Key::A), KeyTransition::Press);
        assert!(matches!(result, MatchResult::Matched { .. }));

        let result = dispatcher.process(Hotkey::new(Key::A), KeyTransition::Press);
        assert!(matches!(result, MatchResult::Throttled { .. }));

        let result = dispatcher.process(Hotkey::new(Key::B), KeyTransition::Press);
        assert!(matches!(result, MatchResult::Matched { .. }));
    }

    // Rate limit tests

    #[test]
    fn rate_limit_allows_up_to_max_count() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register_with_options(
                Hotkey::new(Key::A),
                Action::Suppress,
                BindingOptions::default()
                    .with_rate_limit(RateLimit::new(3, Duration::from_secs(1))),
            )
            .unwrap();

        for _ in 0..3 {
            let result = dispatcher.process(Hotkey::new(Key::A), KeyTransition::Press);
            assert!(matches!(result, MatchResult::Matched { .. }));
        }

        let result = dispatcher.process(Hotkey::new(Key::A), KeyTransition::Press);
        assert!(matches!(result, MatchResult::Throttled { .. }));
    }

    #[test]
    fn rate_limit_resets_after_window() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register_with_options(
                Hotkey::new(Key::A),
                Action::Suppress,
                BindingOptions::default()
                    .with_rate_limit(RateLimit::new(2, Duration::from_millis(10))),
            )
            .unwrap();

        for _ in 0..2 {
            let result = dispatcher.process(Hotkey::new(Key::A), KeyTransition::Press);
            assert!(matches!(result, MatchResult::Matched { .. }));
        }

        let result = dispatcher.process(Hotkey::new(Key::A), KeyTransition::Press);
        assert!(matches!(result, MatchResult::Throttled { .. }));

        std::thread::sleep(Duration::from_millis(15));

        let result = dispatcher.process(Hotkey::new(Key::A), KeyTransition::Press);
        assert!(matches!(result, MatchResult::Matched { .. }));
    }

    // RepeatPolicy in MatchResult tests

    #[test]
    fn matched_result_carries_repeat_policy() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register_with_options(
                Hotkey::new(Key::A),
                Action::Suppress,
                BindingOptions::default().with_repeat_policy(RepeatPolicy::Allow),
            )
            .unwrap();

        let result = dispatcher.process(Hotkey::new(Key::A), KeyTransition::Press);
        match result {
            MatchResult::Matched { repeat_policy, .. } => {
                assert!(matches!(repeat_policy, RepeatPolicy::Allow));
            }
            _ => panic!("expected Matched"),
        }
    }

    #[test]
    fn matched_result_default_repeat_policy_is_suppress() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register(Hotkey::new(Key::A), Action::Suppress)
            .unwrap();

        let result = dispatcher.process(Hotkey::new(Key::A), KeyTransition::Press);
        match result {
            MatchResult::Matched { repeat_policy, .. } => {
                assert!(matches!(repeat_policy, RepeatPolicy::Suppress));
            }
            _ => panic!("expected Matched"),
        }
    }

    // Debounce and repeat interaction

    #[test]
    fn debounce_does_not_affect_repeat_events() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register_with_options(
                Hotkey::new(Key::A),
                Action::Suppress,
                BindingOptions::default()
                    .with_debounce(Duration::from_millis(100))
                    .with_repeat_policy(RepeatPolicy::Allow),
            )
            .unwrap();

        let result = dispatcher.process(Hotkey::new(Key::A), KeyTransition::Press);
        assert!(matches!(result, MatchResult::Matched { .. }));

        let result = dispatcher.process(Hotkey::new(Key::A), KeyTransition::Repeat);
        assert!(matches!(result, MatchResult::Ignored));
    }

    // Throttled result preserves propagation

    #[test]
    fn throttled_result_carries_propagation() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register_with_options(
                Hotkey::new(Key::A),
                Action::Suppress,
                BindingOptions::default()
                    .with_debounce(Duration::from_millis(100))
                    .with_propagation(KeyPropagation::Continue),
            )
            .unwrap();

        dispatcher.process(Hotkey::new(Key::A), KeyTransition::Press);

        let result = dispatcher.process(Hotkey::new(Key::A), KeyTransition::Press);
        match result {
            MatchResult::Throttled { propagation } => {
                assert_eq!(propagation, KeyPropagation::Continue);
            }
            _ => panic!("expected Throttled, got {result:?}"),
        }
    }

    // Tap-hold tests

    #[test]
    fn tap_hold_tap_resolves_on_release_before_threshold() {
        let mut dispatcher = Dispatcher::new();
        let tap_counter = Arc::new(AtomicUsize::new(0));
        let hold_counter = Arc::new(AtomicUsize::new(0));
        let tc = Arc::clone(&tap_counter);
        let hc = Arc::clone(&hold_counter);

        dispatcher
            .register_tap_hold(
                Key::CAPS_LOCK,
                Action::from(move || {
                    tc.fetch_add(1, Ordering::Relaxed);
                }),
                Action::from(move || {
                    hc.fetch_add(1, Ordering::Relaxed);
                }),
                TapHoldOptions::default(),
            )
            .unwrap();

        // Press CapsLock — consumed (buffered)
        let press_result = dispatcher.process(Hotkey::new(Key::CAPS_LOCK), KeyTransition::Press);
        assert!(matches!(press_result, MatchResult::Matched { .. }));
        assert_eq!(tap_counter.load(Ordering::Relaxed), 0);
        assert_eq!(hold_counter.load(Ordering::Relaxed), 0);

        // Release before threshold — tap action resolves
        let release_result =
            dispatcher.process(Hotkey::new(Key::CAPS_LOCK), KeyTransition::Release);
        assert!(matches!(release_result, MatchResult::Matched { .. }));
        if let MatchResult::Matched {
            action: Action::Callback(cb),
            ..
        } = release_result
        {
            cb();
        }
        assert_eq!(tap_counter.load(Ordering::Relaxed), 1);
        assert_eq!(hold_counter.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn tap_hold_hold_resolves_on_threshold_expiry() {
        let mut dispatcher = Dispatcher::new();
        let tap_counter = Arc::new(AtomicUsize::new(0));
        let hold_counter = Arc::new(AtomicUsize::new(0));
        let tc = Arc::clone(&tap_counter);
        let hc = Arc::clone(&hold_counter);

        dispatcher
            .register_tap_hold(
                Key::CAPS_LOCK,
                Action::from(move || {
                    tc.fetch_add(1, Ordering::Relaxed);
                }),
                Action::from(move || {
                    hc.fetch_add(1, Ordering::Relaxed);
                }),
                TapHoldOptions::new().with_threshold(Duration::from_millis(50)),
            )
            .unwrap();

        dispatcher.process(Hotkey::new(Key::CAPS_LOCK), KeyTransition::Press);
        std::thread::sleep(Duration::from_millis(60));

        let pending = dispatcher.pending_timeouts();
        assert!(!pending.is_empty());
        let mut found_match = false;
        for p in &pending {
            if let Some(MatchResult::Matched {
                action: Action::Callback(cb),
                ..
            }) = dispatcher.match_pending_timeout(p)
            {
                cb();
                found_match = true;
            }
        }
        assert!(found_match);
        assert_eq!(tap_counter.load(Ordering::Relaxed), 0);
        assert_eq!(hold_counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn tap_hold_hold_resolves_on_interrupting_keypress() {
        let mut dispatcher = Dispatcher::new();
        let tap_counter = Arc::new(AtomicUsize::new(0));
        let hold_counter = Arc::new(AtomicUsize::new(0));
        let tc = Arc::clone(&tap_counter);
        let hc = Arc::clone(&hold_counter);

        dispatcher
            .register_tap_hold(
                Key::CAPS_LOCK,
                Action::from(move || {
                    tc.fetch_add(1, Ordering::Relaxed);
                }),
                Action::from(move || {
                    hc.fetch_add(1, Ordering::Relaxed);
                }),
                TapHoldOptions::default(),
            )
            .unwrap();

        dispatcher.process(Hotkey::new(Key::CAPS_LOCK), KeyTransition::Press);

        // Interrupt with A — CapsLock is marked resolved immediately,
        // but the hold action is deferred to the pending_timeouts pipeline.
        let _interrupt = dispatcher.process(Hotkey::new(Key::A), KeyTransition::Press);
        assert_eq!(hold_counter.load(Ordering::Relaxed), 0);

        // Drain pending timeouts — hold action surfaces here.
        let pending = dispatcher.pending_timeouts();
        assert!(!pending.is_empty());
        for p in &pending {
            if let Some(MatchResult::Matched {
                action: Action::Callback(cb),
                ..
            }) = dispatcher.match_pending_timeout(p)
            {
                cb();
            }
        }

        assert_eq!(tap_counter.load(Ordering::Relaxed), 0);
        assert_eq!(hold_counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn tap_hold_can_be_reused_after_tap() {
        let mut dispatcher = Dispatcher::new();
        let tap_counter = Arc::new(AtomicUsize::new(0));
        let hold_counter = Arc::new(AtomicUsize::new(0));
        let tc = Arc::clone(&tap_counter);
        let hc = Arc::clone(&hold_counter);

        dispatcher
            .register_tap_hold(
                Key::CAPS_LOCK,
                Action::from(move || {
                    tc.fetch_add(1, Ordering::Relaxed);
                }),
                Action::from(move || {
                    hc.fetch_add(1, Ordering::Relaxed);
                }),
                TapHoldOptions::default(),
            )
            .unwrap();

        // First tap
        dispatcher.process(Hotkey::new(Key::CAPS_LOCK), KeyTransition::Press);
        let result = dispatcher.process(Hotkey::new(Key::CAPS_LOCK), KeyTransition::Release);
        if let MatchResult::Matched {
            action: Action::Callback(cb),
            ..
        } = result
        {
            cb();
        }
        assert_eq!(tap_counter.load(Ordering::Relaxed), 1);

        // Second tap
        dispatcher.process(Hotkey::new(Key::CAPS_LOCK), KeyTransition::Press);
        let result = dispatcher.process(Hotkey::new(Key::CAPS_LOCK), KeyTransition::Release);
        if let MatchResult::Matched {
            action: Action::Callback(cb),
            ..
        } = result
        {
            cb();
        }
        assert_eq!(tap_counter.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn tap_hold_repeat_events_are_consumed() {
        let mut dispatcher = Dispatcher::new();

        dispatcher
            .register_tap_hold(
                Key::CAPS_LOCK,
                Action::Suppress,
                Action::Suppress,
                TapHoldOptions::default(),
            )
            .unwrap();

        dispatcher.process(Hotkey::new(Key::CAPS_LOCK), KeyTransition::Press);

        let result = dispatcher.process(Hotkey::new(Key::CAPS_LOCK), KeyTransition::Repeat);
        assert!(!matches!(
            result,
            MatchResult::NoMatch | MatchResult::Ignored
        ));
    }

    #[test]
    fn tap_hold_non_registered_key_passes_through() {
        let mut dispatcher = Dispatcher::new();

        dispatcher
            .register_tap_hold(
                Key::CAPS_LOCK,
                Action::Suppress,
                Action::Suppress,
                TapHoldOptions::default(),
            )
            .unwrap();

        let result = dispatcher.process(Hotkey::new(Key::A), KeyTransition::Press);
        assert!(matches!(result, MatchResult::NoMatch));
    }

    #[test]
    fn tap_hold_timeout_deadline_reported() {
        let mut dispatcher = Dispatcher::new();

        dispatcher
            .register_tap_hold(
                Key::CAPS_LOCK,
                Action::Suppress,
                Action::Suppress,
                TapHoldOptions::new().with_threshold(Duration::from_millis(200)),
            )
            .unwrap();

        assert!(dispatcher.next_timeout_deadline().is_none());

        dispatcher.process(Hotkey::new(Key::CAPS_LOCK), KeyTransition::Press);
        let deadline = dispatcher.next_timeout_deadline();
        assert!(deadline.is_some());
        assert!(deadline.unwrap() <= Duration::from_millis(201));
    }

    #[test]
    fn tap_hold_duplicate_registration_rejected() {
        let mut dispatcher = Dispatcher::new();

        dispatcher
            .register_tap_hold(
                Key::CAPS_LOCK,
                Action::Suppress,
                Action::Suppress,
                TapHoldOptions::default(),
            )
            .unwrap();

        let result = dispatcher.register_tap_hold(
            Key::CAPS_LOCK,
            Action::Suppress,
            Action::Suppress,
            TapHoldOptions::default(),
        );
        assert!(matches!(
            result,
            Err(crate::error::RegisterError::AlreadyRegistered)
        ));
    }

    #[test]
    fn tap_hold_unregister_stops_tap_hold_behavior() {
        let mut dispatcher = Dispatcher::new();
        let tap_counter = Arc::new(AtomicUsize::new(0));
        let tc = Arc::clone(&tap_counter);

        let id = dispatcher
            .register_tap_hold(
                Key::CAPS_LOCK,
                Action::from(move || {
                    tc.fetch_add(1, Ordering::Relaxed);
                }),
                Action::Suppress,
                TapHoldOptions::default(),
            )
            .unwrap();

        // Tap works before unregister
        dispatcher.process(Hotkey::new(Key::CAPS_LOCK), KeyTransition::Press);
        let result = dispatcher.process(Hotkey::new(Key::CAPS_LOCK), KeyTransition::Release);
        assert!(matches!(result, MatchResult::Matched { .. }));

        // Unregister
        dispatcher.unregister(id);

        // Key should pass through now
        let result = dispatcher.process(Hotkey::new(Key::CAPS_LOCK), KeyTransition::Press);
        assert!(matches!(result, MatchResult::NoMatch));
    }

    #[test]
    fn tap_hold_unregister_clears_active_state() {
        let mut dispatcher = Dispatcher::new();

        let id = dispatcher
            .register_tap_hold(
                Key::CAPS_LOCK,
                Action::Suppress,
                Action::Suppress,
                TapHoldOptions::new().with_threshold(Duration::from_millis(200)),
            )
            .unwrap();

        // Press to create active state
        dispatcher.process(Hotkey::new(Key::CAPS_LOCK), KeyTransition::Press);
        assert!(dispatcher.next_timeout_deadline().is_some());

        // Unregister while key is active
        dispatcher.unregister(id);

        // No more pending deadlines
        assert!(dispatcher.next_timeout_deadline().is_none());

        // Release should pass through (no active tap-hold state)
        let result = dispatcher.process(Hotkey::new(Key::CAPS_LOCK), KeyTransition::Release);
        assert!(matches!(result, MatchResult::Ignored));
    }

    #[test]
    fn tap_hold_can_reregister_after_unregister() {
        let mut dispatcher = Dispatcher::new();

        let id = dispatcher
            .register_tap_hold(
                Key::CAPS_LOCK,
                Action::Suppress,
                Action::Suppress,
                TapHoldOptions::default(),
            )
            .unwrap();

        dispatcher.unregister(id);

        // Should be able to re-register the same key
        let result = dispatcher.register_tap_hold(
            Key::CAPS_LOCK,
            Action::Suppress,
            Action::Suppress,
            TapHoldOptions::default(),
        );
        assert!(result.is_ok());
    }
}
