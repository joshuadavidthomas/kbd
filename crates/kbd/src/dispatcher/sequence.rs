use std::time::Duration;
use std::time::Instant;

use super::BindingMatch;
use super::Dispatcher;
use super::MatchedBindingRef;
use super::layers::LayerEffect;
use super::timeout::PendingTimeout;
use super::timeout::TimeoutKind;
use crate::binding::BindingId;
use crate::layer::LayerName;
use crate::observation::KeyboardObservation;
use crate::policy::KeyPropagation;
use crate::policy::RepeatPolicy;
use crate::sequence::PendingSequenceInfo;
use crate::sequence::SequenceOptions;

pub(super) enum SequenceBindingRef {
    Global(BindingId),
    Layer {
        id: BindingId,
        name: LayerName,
        index: usize,
    },
}

pub(super) struct ActiveSequence {
    pub(super) binding_ref: SequenceBindingRef,
    pub(super) next_step_index: usize,
    pub(super) deadline: Instant,
    pub(super) priority: usize,
}

/// The match data shared between a pending standalone and a fired timeout.
///
/// This is the core of a deferred standalone binding: everything needed
/// to resolve it to an action, minus the layer effect (which is applied
/// during the pending → fired transition).
pub(super) struct StandaloneMatch {
    pub(super) binding_ref: MatchedBindingRef,
    pub(super) propagation: KeyPropagation,
    pub(super) repeat_policy: RepeatPolicy,
}

/// A standalone binding deferred while sequences are in progress.
///
/// Contains a [`StandaloneMatch`] plus a [`LayerEffect`] that will be
/// applied when the standalone fires (only on sequence timeout).
pub(super) struct PendingStandalone {
    pub(super) inner: StandaloneMatch,
    pub(super) layer_effect: LayerEffect,
}

pub(super) enum SequenceStartCandidate {
    SingleStep {
        binding_ref: MatchedBindingRef,
        layer_effect: LayerEffect,
        propagation: KeyPropagation,
    },
    MultiStep {
        binding_ref: SequenceBindingRef,
        timeout: Duration,
    },
}

impl Dispatcher {
    /// Cancel pending sequence candidates and their deferred standalone fallback.
    ///
    /// Idempotent and silent: no actions, layer effects, or synthetic releases.
    /// Registrations, layers, tap-hold state, and throttle history are unchanged.
    /// This is not a full input reset. Collected sequence timeout tokens are revoked.
    pub fn cancel_pending_sequence(&mut self) {
        self.active_sequences.clear();
        self.pending_standalone = None;
        self.input_epoch += 1;
    }

    pub(super) fn match_active_sequences(
        &mut self,
        event: &KeyboardObservation,
    ) -> Option<BindingMatch> {
        if self.active_sequences.is_empty() {
            return None;
        }

        let now = Instant::now();
        let mut survivors = Vec::new();
        let mut completed: Vec<(usize, SequenceBindingRef)> = Vec::new();
        let mut expired = false;
        let mut aborted = false;
        let mut mismatched = false;
        let active_sequences = std::mem::take(&mut self.active_sequences);

        for mut active in active_sequences {
            if active.deadline <= now {
                expired = true;
                continue;
            }

            if self.sequence_step_matches(&active.binding_ref, active.next_step_index, event) {
                active.next_step_index += 1;
                let total = self.sequence_step_count(&active.binding_ref);
                if active.next_step_index >= total {
                    completed.push((active.priority, active.binding_ref));
                } else {
                    active.deadline = now + self.sequence_options(&active.binding_ref).timeout();
                    survivors.push(active);
                }
                continue;
            }

            if self
                .sequence_options(&active.binding_ref)
                .abort_key()
                .matches(event)
            {
                aborted = true;
            } else {
                mismatched = true;
            }
        }

        if let Some((_, sequence_ref)) = completed.into_iter().min_by_key(|(priority, _)| *priority)
        {
            self.active_sequences.clear();
            self.pending_standalone = None;
            return Some(self.matched_outcome_for_sequence(sequence_ref));
        }

        if !survivors.is_empty() {
            self.active_sequences = survivors;
            // The standalone fallback only applies while waiting on step 2
            // after the initial sequence prefix keypress. Once the user has
            // progressed the sequence, timing out should not retroactively fire
            // that first-step standalone action.
            self.pending_standalone = None;
            if let Some(pending) = self.pending_sequence_snapshot() {
                return Some(BindingMatch::Pending {
                    steps_matched: pending.steps_matched,
                    steps_remaining: pending.steps_remaining,
                });
            }
            return Some(BindingMatch::NoMatch);
        }

        self.active_sequences.clear();

        if aborted {
            self.pending_standalone = None;
            return Some(BindingMatch::NoMatch);
        }

        if let Some(standalone) = self
            .pending_standalone
            .take()
            .filter(|_| expired && !mismatched)
        {
            return Some(BindingMatch::Matched {
                binding_ref: standalone.inner.binding_ref,
                layer_effect: standalone.layer_effect,
                propagation: standalone.inner.propagation,
                repeat_policy: standalone.inner.repeat_policy,
            });
        }

        self.pending_standalone = None;
        None
    }

    pub(super) fn start_sequences(
        &mut self,
        candidates: Vec<SequenceStartCandidate>,
        now: Instant,
        next_priority: &mut usize,
        pending_standalone: Option<PendingStandalone>,
    ) -> Option<BindingMatch> {
        if candidates.is_empty() {
            return None;
        }

        let mut started = Vec::new();
        for candidate in candidates {
            match candidate {
                SequenceStartCandidate::SingleStep {
                    binding_ref,
                    layer_effect,
                    propagation,
                } => {
                    self.active_sequences.clear();
                    self.pending_standalone = None;
                    return Some(BindingMatch::Matched {
                        binding_ref,
                        layer_effect,
                        propagation,
                        repeat_policy: RepeatPolicy::default(),
                    });
                }
                SequenceStartCandidate::MultiStep {
                    binding_ref,
                    timeout,
                } => {
                    started.push(ActiveSequence {
                        binding_ref,
                        next_step_index: 1,
                        deadline: now + timeout,
                        priority: *next_priority,
                    });
                    *next_priority += 1;
                }
            }
        }

        self.active_sequences = started;
        self.pending_standalone = pending_standalone;

        if let Some(pending) = self.pending_sequence_snapshot() {
            return Some(BindingMatch::Pending {
                steps_matched: pending.steps_matched,
                steps_remaining: pending.steps_remaining,
            });
        }

        Some(BindingMatch::NoMatch)
    }

    pub(super) fn pending_standalone_from_match(
        &self,
        binding_match: Option<(MatchedBindingRef, KeyPropagation, RepeatPolicy)>,
    ) -> Option<PendingStandalone> {
        binding_match.map(|(binding_ref, propagation, repeat_policy)| {
            let layer_effect = LayerEffect::from_action(self.resolve_binding(&binding_ref));
            PendingStandalone {
                inner: StandaloneMatch {
                    binding_ref,
                    propagation,
                    repeat_policy,
                },
                layer_effect,
            }
        })
    }

    pub(super) fn check_sequence_timeouts(&mut self, now: Instant) -> Option<PendingTimeout> {
        if self.active_sequences.is_empty() {
            return None;
        }

        let before = self.active_sequences.len();
        self.active_sequences.retain(|active| active.deadline > now);
        let expired = before.saturating_sub(self.active_sequences.len());

        if expired > 0 && self.active_sequences.is_empty() {
            if let Some(standalone) = self.pending_standalone.take() {
                self.apply_layer_effect(&standalone.layer_effect);
                return Some(PendingTimeout {
                    kind: TimeoutKind::Standalone(standalone.inner),
                    epoch: self.input_epoch,
                });
            }

            self.pending_standalone = None;
        }

        None
    }

    fn sequence_step_count(&self, binding_ref: &SequenceBindingRef) -> usize {
        match binding_ref {
            SequenceBindingRef::Global(id) => {
                self.sequence_bindings_by_id[id].sequence.steps().len()
            }
            SequenceBindingRef::Layer { name, index, .. } => self.layers[name].sequence_bindings
                [*index]
                .sequence
                .steps()
                .len(),
        }
    }

    fn sequence_step_matches(
        &self,
        binding_ref: &SequenceBindingRef,
        step_index: usize,
        event: &KeyboardObservation,
    ) -> bool {
        match binding_ref {
            SequenceBindingRef::Global(id) => self.sequence_bindings_by_id[id]
                .sequence
                .steps()
                .get(step_index)
                .is_some_and(|step| step.matches(event)),
            SequenceBindingRef::Layer { name, index, .. } => self.layers[name].sequence_bindings
                [*index]
                .sequence
                .steps()
                .get(step_index)
                .is_some_and(|step| step.matches(event)),
        }
    }

    fn sequence_options(&self, binding_ref: &SequenceBindingRef) -> &SequenceOptions {
        match binding_ref {
            SequenceBindingRef::Global(id) => &self.sequence_bindings_by_id[id].options,
            SequenceBindingRef::Layer { name, index, .. } => {
                &self.layers[name].sequence_bindings[*index].options
            }
        }
    }

    fn matched_outcome_for_sequence(&self, sequence_ref: SequenceBindingRef) -> BindingMatch {
        match sequence_ref {
            SequenceBindingRef::Global(id) => {
                let binding = &self.sequence_bindings_by_id[&id];
                BindingMatch::Matched {
                    binding_ref: MatchedBindingRef::SequenceGlobal(id),
                    layer_effect: LayerEffect::from_action(&binding.action),
                    propagation: binding.propagation,
                    repeat_policy: RepeatPolicy::default(),
                }
            }
            SequenceBindingRef::Layer { id, name, index } => {
                let binding = &self.layers[&name].sequence_bindings[index];
                BindingMatch::Matched {
                    binding_ref: MatchedBindingRef::SequenceLayer { id, name, index },
                    layer_effect: LayerEffect::from_action(&binding.action),
                    propagation: binding.propagation,
                    repeat_policy: RepeatPolicy::default(),
                }
            }
        }
    }

    pub(super) fn pending_sequence_snapshot(&self) -> Option<PendingSequenceInfo> {
        self.active_sequences
            .iter()
            .max_by_key(|active| (active.next_step_index, std::cmp::Reverse(active.priority)))
            .map(|active| {
                let total = self.sequence_step_count(&active.binding_ref);
                PendingSequenceInfo {
                    steps_matched: active.next_step_index,
                    steps_remaining: total.saturating_sub(active.next_step_index),
                }
            })
    }

    pub(super) fn clear_sequences_for_layer_if_inactive(&mut self, layer_name: &LayerName) {
        if self
            .layer_stack
            .iter()
            .any(|entry| &entry.name == layer_name)
        {
            return;
        }

        self.clear_sequences_for_layer(layer_name);
    }

    pub(super) fn clear_sequences_for_layer(&mut self, layer_name: &LayerName) {
        self.active_sequences.retain(|active| {
            !matches!(
                active.binding_ref,
                SequenceBindingRef::Layer { ref name, .. } if name == layer_name
            )
        });

        if self.pending_standalone.as_ref().is_some_and(|pending| {
            matches!(
                &pending.inner.binding_ref,
                MatchedBindingRef::Layer { name, .. }
                    | MatchedBindingRef::SequenceLayer { name, .. }
                    if name == layer_name
            )
        }) {
            self.pending_standalone = None;
        }

        if self.active_sequences.is_empty() {
            self.pending_standalone = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    use super::super::Dispatcher;
    use super::super::MatchResult;
    use crate::action::Action;
    use crate::hotkey::Hotkey;
    use crate::hotkey::HotkeySequence;
    use crate::hotkey::Modifier;
    use crate::key::Key;
    use crate::key_state::KeyTransition;
    use crate::layer::Layer;
    use crate::layer::LayerName;
    use crate::sequence::SequenceOptions;

    fn execute_callback(result: &MatchResult<'_>) {
        if let MatchResult::Matched {
            action: Action::Callback(callback),
            ..
        } = result
        {
            callback();
        }
    }

    fn fire_pending_timeouts(dispatcher: &mut Dispatcher) {
        for pending in &dispatcher.pending_timeouts() {
            if let Some(result) = dispatcher.match_pending_timeout(pending) {
                execute_callback(&result);
            }
        }
    }

    #[test]
    fn sequence_reports_pending_then_fires_on_completion() {
        let mut dispatcher = Dispatcher::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        dispatcher
            .register_sequence(
                "Ctrl+K, Ctrl+C".parse::<HotkeySequence>().unwrap(),
                move || {
                    cc.fetch_add(1, Ordering::Relaxed);
                },
            )
            .unwrap();

        let first = dispatcher.process(
            Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(first, MatchResult::Pending { .. }));

        let second = dispatcher.process(
            Hotkey::new(Key::C).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        execute_callback(&second);

        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn sequence_timeout_fires_standalone_binding() {
        let mut dispatcher = Dispatcher::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        dispatcher
            .register(Hotkey::new(Key::K).modifier(Modifier::Ctrl), move || {
                cc.fetch_add(1, Ordering::Relaxed);
            })
            .unwrap();
        dispatcher
            .register_sequence_with_options(
                "Ctrl+K, Ctrl+C".parse::<HotkeySequence>().unwrap(),
                || {},
                SequenceOptions::default().with_timeout(Duration::from_millis(10)),
            )
            .unwrap();

        let first = dispatcher.process(
            Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(first, MatchResult::Pending { .. }));

        std::thread::sleep(Duration::from_millis(20));
        fire_pending_timeouts(&mut dispatcher);

        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn sequence_wrong_key_resets_and_current_key_re_matches() {
        let mut dispatcher = Dispatcher::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        dispatcher
            .register_sequence("Ctrl+K, Ctrl+C".parse::<HotkeySequence>().unwrap(), || {})
            .unwrap();
        dispatcher
            .register(Hotkey::new(Key::X).modifier(Modifier::Ctrl), move || {
                cc.fetch_add(1, Ordering::Relaxed);
            })
            .unwrap();

        let first = dispatcher.process(
            Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(first, MatchResult::Pending { .. }));

        let wrong = dispatcher.process(
            Hotkey::new(Key::X).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        execute_callback(&wrong);

        assert_eq!(counter.load(Ordering::Relaxed), 1);
        assert!(dispatcher.pending_sequence().is_none());
    }

    #[test]
    fn abort_key_cancels_pending_sequence() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register_sequence("Ctrl+K, Ctrl+C".parse::<HotkeySequence>().unwrap(), || {})
            .unwrap();

        let first = dispatcher.process(
            Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(first, MatchResult::Pending { .. }));

        let aborted = dispatcher.process(Hotkey::new(Key::ESCAPE), KeyTransition::Press);
        assert!(matches!(aborted, MatchResult::NoMatch));
        assert!(dispatcher.pending_sequence().is_none());
    }

    #[test]
    fn abort_key_step_can_still_complete_sequence() {
        let mut dispatcher = Dispatcher::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        dispatcher
            .register_sequence(
                "Ctrl+K, Escape".parse::<HotkeySequence>().unwrap(),
                move || {
                    cc.fetch_add(1, Ordering::Relaxed);
                },
            )
            .unwrap();

        let first = dispatcher.process(
            Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(first, MatchResult::Pending { .. }));

        let second = dispatcher.process(Hotkey::new(Key::ESCAPE), KeyTransition::Press);
        execute_callback(&second);

        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn overlapping_prefix_falls_back_to_standalone_on_timeout() {
        let mut dispatcher = Dispatcher::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        dispatcher
            .register(Hotkey::new(Key::K).modifier(Modifier::Ctrl), move || {
                cc.fetch_add(1, Ordering::Relaxed);
            })
            .unwrap();
        dispatcher
            .register_sequence_with_options(
                "Ctrl+K, Ctrl+C".parse::<HotkeySequence>().unwrap(),
                || {},
                SequenceOptions::default().with_timeout(Duration::from_millis(10)),
            )
            .unwrap();

        let pending = dispatcher.process(
            Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(pending, MatchResult::Pending { .. }));

        std::thread::sleep(Duration::from_millis(20));
        fire_pending_timeouts(&mut dispatcher);

        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn timeout_after_sequence_progress_does_not_fire_first_step_fallback() {
        let mut dispatcher = Dispatcher::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        dispatcher
            .register(Hotkey::new(Key::K).modifier(Modifier::Ctrl), move || {
                cc.fetch_add(1, Ordering::Relaxed);
            })
            .unwrap();
        dispatcher
            .register_sequence_with_options(
                "Ctrl+K, Ctrl+S, Ctrl+C".parse::<HotkeySequence>().unwrap(),
                || {},
                SequenceOptions::default().with_timeout(Duration::from_millis(10)),
            )
            .unwrap();

        let first = dispatcher.process(
            Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(first, MatchResult::Pending { .. }));

        let second = dispatcher.process(
            Hotkey::new(Key::S).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(second, MatchResult::Pending { .. }));

        std::thread::sleep(Duration::from_millis(20));
        fire_pending_timeouts(&mut dispatcher);

        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn multiple_sequences_with_shared_prefix_progress_independently() {
        let mut dispatcher = Dispatcher::new();
        let c_counter = Arc::new(AtomicUsize::new(0));
        let d_counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&c_counter);
        let dc = Arc::clone(&d_counter);

        dispatcher
            .register_sequence(
                "Ctrl+K, Ctrl+S, Ctrl+C".parse::<HotkeySequence>().unwrap(),
                move || {
                    cc.fetch_add(1, Ordering::Relaxed);
                },
            )
            .unwrap();
        dispatcher
            .register_sequence(
                "Ctrl+K, Ctrl+S, Ctrl+D".parse::<HotkeySequence>().unwrap(),
                move || {
                    dc.fetch_add(1, Ordering::Relaxed);
                },
            )
            .unwrap();

        let first = dispatcher.process(
            Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(first, MatchResult::Pending { .. }));
        let second = dispatcher.process(
            Hotkey::new(Key::S).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(second, MatchResult::Pending { .. }));

        let third = dispatcher.process(
            Hotkey::new(Key::D).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        execute_callback(&third);

        assert_eq!(c_counter.load(Ordering::Relaxed), 0);
        assert_eq!(d_counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn layer_timeout_clears_pending_layer_sequence_state() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .define_layer(
                Layer::new("timed")
                    .bind_sequence("Ctrl+K, Ctrl+C", Action::Suppress)
                    .unwrap()
                    .timeout(Duration::from_millis(10)),
            )
            .unwrap();
        dispatcher.push_layer("timed").unwrap();

        let first = dispatcher.process(
            Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(first, MatchResult::Pending { .. }));

        std::thread::sleep(Duration::from_millis(20));
        let _ = dispatcher.pending_timeouts();

        assert!(dispatcher.pending_sequence().is_none());
    }

    #[test]
    fn popping_one_of_duplicate_layers_keeps_pending_sequence_state() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .define_layer(
                Layer::new("nav")
                    .bind_sequence("Ctrl+K, Ctrl+C", Action::Suppress)
                    .unwrap()
                    .swallow(),
            )
            .unwrap();
        dispatcher.push_layer("nav").unwrap();
        dispatcher.push_layer("nav").unwrap();

        let first = dispatcher.process(
            Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(first, MatchResult::Pending { .. }));

        dispatcher.pop_layer().unwrap();

        let pending = dispatcher
            .pending_sequence()
            .expect("sequence should remain pending");
        assert_eq!(pending.steps_matched, 1);
        assert_eq!(pending.steps_remaining, 1);

        let second = dispatcher.process(
            Hotkey::new(Key::C).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(second, MatchResult::Matched { .. }));
    }

    #[test]
    fn unregistering_standalone_while_sequence_pending_does_not_panic_or_fire_fallback() {
        let mut dispatcher = Dispatcher::new();

        let standalone_id = dispatcher
            .register(
                Hotkey::new(Key::K).modifier(Modifier::Ctrl),
                Action::Suppress,
            )
            .unwrap();
        dispatcher
            .register_sequence_with_options(
                "Ctrl+K, Ctrl+C".parse::<HotkeySequence>().unwrap(),
                Action::Suppress,
                SequenceOptions::default().with_timeout(Duration::from_millis(10)),
            )
            .unwrap();

        let first = dispatcher.process(
            Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(first, MatchResult::Pending { .. }));

        dispatcher.unregister(standalone_id);

        std::thread::sleep(Duration::from_millis(20));
        let pending = dispatcher.pending_timeouts();
        assert!(pending.is_empty());
    }

    #[test]
    fn timeout_fallback_applies_layer_effects() {
        let mut dispatcher = Dispatcher::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        dispatcher
            .define_layer(
                Layer::new("nav")
                    .bind(Key::H, move || {
                        cc.fetch_add(1, Ordering::Relaxed);
                    })
                    .unwrap(),
            )
            .unwrap();
        dispatcher
            .register(
                Hotkey::new(Key::K).modifier(Modifier::Ctrl),
                Action::PushLayer(LayerName::from("nav")),
            )
            .unwrap();
        dispatcher
            .register_sequence_with_options(
                "Ctrl+K, Ctrl+C".parse::<HotkeySequence>().unwrap(),
                Action::Suppress,
                SequenceOptions::default().with_timeout(Duration::from_millis(10)),
            )
            .unwrap();

        let first = dispatcher.process(
            Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(first, MatchResult::Pending { .. }));

        std::thread::sleep(Duration::from_millis(20));
        fire_pending_timeouts(&mut dispatcher);

        let h = dispatcher.process(Hotkey::new(Key::H), KeyTransition::Press);
        execute_callback(&h);

        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn pending_sequence_query_reports_progress() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register_sequence("Ctrl+K, Ctrl+C".parse::<HotkeySequence>().unwrap(), || {})
            .unwrap();

        let first = dispatcher.process(
            Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(first, MatchResult::Pending { .. }));

        let pending = dispatcher.pending_sequence().expect("pending sequence");
        assert_eq!(pending.steps_matched, 1);
        assert_eq!(pending.steps_remaining, 1);
    }
}

#[cfg(test)]
mod observation_tests {
    use super::*;
    use crate::action::Action;
    use crate::binding::BindingOptions;
    use crate::dispatcher::MatchResult;
    use crate::hotkey::Hotkey;
    use crate::hotkey::ModifierSet;
    use crate::key::Key;
    use crate::key_state::KeyTransition;
    use crate::layer::Layer;
    use crate::observation::BindingPattern;
    use crate::observation::LogicalKeyValue;
    use crate::observation::NamedKey;
    use crate::sequence::BindingSequence;
    use crate::sequence::SequenceAbortKey;

    fn p(key: Key) -> BindingPattern {
        Hotkey::new(key).into()
    }
    fn c(text: &str) -> BindingPattern {
        BindingPattern::logical(LogicalKeyValue::Character(text.into()), ModifierSet::NONE)
    }
    fn seq(steps: Vec<BindingPattern>) -> BindingSequence {
        BindingSequence::new(steps).unwrap()
    }
    fn event(physical: Option<Key>, logical: Option<&str>) -> KeyboardObservation {
        KeyboardObservation {
            physical,
            logical: logical.map(|text| LogicalKeyValue::Character(text.into()).into()),
            modifiers: ModifierSet::NONE,
            modifier_observation: None,
            transition: KeyTransition::Press,
        }
    }
    fn emit(key: Key) -> Action {
        Action::EmitHotkey(Hotkey::new(key))
    }
    fn emitted(result: MatchResult<'_>) -> Key {
        match result {
            MatchResult::Matched {
                action: Action::EmitHotkey(hotkey),
                ..
            } => hotkey.key(),
            other => panic!("expected emitted action, got {other:?}"),
        }
    }
    fn install(d: &mut Dispatcher, in_layer: bool, bindings: Vec<(BindingSequence, Key)>) {
        if in_layer {
            let mut layer = Layer::new("mode");
            for (sequence, action) in bindings {
                layer =
                    layer.bind_sequence_pattern(sequence, emit(action), SequenceOptions::default());
            }
            d.define_layer(layer).unwrap();
            d.push_layer("mode").unwrap();
        } else {
            for (sequence, action) in bindings {
                d.register_sequence_pattern(sequence, emit(action), SequenceOptions::default())
                    .unwrap();
            }
        }
    }

    #[test]
    fn mixed_sequences_advance_once_and_queries_do_not_simulate_pending() {
        for in_layer in [false, true] {
            for steps in [vec![p(Key::A), c("a")], vec![c("a"), p(Key::A)]] {
                let mut d = Dispatcher::new();
                install(&mut d, in_layer, vec![(seq(steps), Key::Z)]);
                let mut e = event(Some(Key::A), Some("a"));
                assert!(d.bindings_for_event(&e).is_none());
                assert!(d.pending_sequence().is_none());
                assert!(matches!(
                    d.process_event(&e),
                    MatchResult::Pending {
                        steps_matched: 1,
                        steps_remaining: 1
                    }
                ));
                let deadline = d.active_sequences[0].deadline;
                assert!(d.bindings_for_event(&e).is_none());
                assert_eq!(d.active_sequences[0].deadline, deadline);
                for transition in [KeyTransition::Repeat, KeyTransition::Release] {
                    e.transition = transition;
                    assert!(matches!(d.process_event(&e), MatchResult::Ignored));
                    assert_eq!(d.pending_sequence().unwrap().steps_matched, 1);
                }
                e.transition = KeyTransition::Press;
                assert_eq!(emitted(d.process_event(&e)), Key::Z);
                assert!(d.pending_sequence().is_none());
            }
        }
    }

    #[test]
    fn all_prefix_domains_survive_and_earliest_differing_step_breaks_completion_ties() {
        for in_layer in [false, true] {
            for reverse in [false, true] {
                let mut d = Dispatcher::new();
                let mut bindings = vec![
                    (seq(vec![p(Key::K), c("c")]), Key::X),
                    (seq(vec![c("k"), p(Key::C)]), Key::Y),
                ];
                if reverse {
                    bindings.reverse();
                }
                install(&mut d, in_layer, bindings);
                d.process_event(&event(Some(Key::K), Some("k")));
                assert_eq!(d.active_sequences.len(), 2);
                assert_eq!(
                    emitted(d.process_event(&event(Some(Key::C), Some("c")))),
                    Key::X
                );
                d.process_event(&event(Some(Key::K), Some("k")));
                // Physical prefix's continuation fails; the logical prefix must still win.
                assert_eq!(
                    emitted(d.process_event(&event(Some(Key::C), Some("other")))),
                    Key::Y
                );
            }
        }
    }

    #[test]
    fn sequence_scope_and_kind_precede_domain() {
        for top_logical in [false, true] {
            let mut d = Dispatcher::new();
            let (top, lower) = if top_logical {
                (c("k"), p(Key::K))
            } else {
                (p(Key::K), c("k"))
            };
            d.register_sequence_pattern(seq(vec![lower]), emit(Key::X), SequenceOptions::default())
                .unwrap();
            let sequence = seq(vec![top.clone(), c("done")]);
            d.define_layer(
                Layer::new("top")
                    .bind_sequence_pattern(sequence, emit(Key::Y), SequenceOptions::default())
                    .bind_pattern(
                        p(Key::K),
                        emit(Key::Z),
                        BindingOptions::default().with_source("user"),
                    ),
            )
            .unwrap();
            d.push_layer("top").unwrap();
            let prefix = event(Some(Key::K), Some("k"));
            assert!(d.bindings_for_event(&prefix).is_none());
            assert!(matches!(
                d.process_event(&prefix),
                MatchResult::Pending { .. }
            ));
            assert_eq!(emitted(d.process_event(&event(None, Some("done")))), Key::Y);
        }
        for in_layer in [false, true] {
            let mut d = Dispatcher::new();
            install(
                &mut d,
                in_layer,
                vec![
                    (seq(vec![p(Key::K), p(Key::C)]), Key::X),
                    (seq(vec![c("k")]), Key::Y),
                ],
            );
            let prefix = event(Some(Key::K), Some("k"));
            assert_eq!(d.bindings_for_event(&prefix).unwrap().pattern(), &c("k"));
            assert_eq!(emitted(d.process_event(&prefix)), Key::Y);
            assert!(d.pending_sequence().is_none());
        }
    }

    #[test]
    fn logical_abort_ignores_modifiers_but_expected_step_wins() {
        let named = BindingPattern::logical(NamedKey::Escape, ModifierSet::NONE);
        for next in [p(Key::C), named.clone()] {
            let mut d = Dispatcher::new();
            let completes = next == named;
            d.register_sequence_pattern(
                seq(vec![c("k"), next]),
                emit(Key::X),
                SequenceOptions::default().with_logical_abort_key(NamedKey::Escape),
            )
            .unwrap();
            d.register_pattern(named.clone(), emit(Key::Y), BindingOptions::default())
                .unwrap();
            d.register_pattern(c("k"), emit(Key::Z), BindingOptions::default())
                .unwrap();
            d.process_event(&event(None, Some("k")));
            let mut abort = event(None, None);
            abort.logical = Some(NamedKey::Escape.into());
            if !completes {
                abort.modifiers = ModifierSet::CTRL;
            }
            let result = d.process_event(&abort);
            if completes {
                assert_eq!(emitted(result), Key::X);
            } else {
                assert!(matches!(result, MatchResult::NoMatch));
            }
            assert!(d.pending_sequence().is_none());
            assert!(d.pending_timeouts().is_empty());
        }
        assert_eq!(
            SequenceOptions::default().abort_key(),
            &SequenceAbortKey::Physical(Key::ESCAPE)
        );
        let mut d = Dispatcher::new();
        d.register_sequence_pattern(
            seq(vec![c("k"), p(Key::C)]),
            emit(Key::X),
            SequenceOptions::default(),
        )
        .unwrap();
        d.register_pattern(named, emit(Key::Y), BindingOptions::default())
            .unwrap();
        d.process_event(&event(None, Some("k")));
        let mut logical_escape = event(None, None);
        logical_escape.logical = Some(NamedKey::Escape.into());
        assert_eq!(emitted(d.process_event(&logical_escape)), Key::Y); // mismatch/retry, not default abort
        d.process_event(&event(None, Some("k")));
        assert!(matches!(
            d.process_event(&event(Some(Key::ESCAPE), None)),
            MatchResult::NoMatch
        ));
    }

    #[test]
    fn aborting_one_candidate_does_not_discard_an_advancing_sibling() {
        let mut d = Dispatcher::new();
        d.register_sequence_pattern(
            seq(vec![c("k"), p(Key::C)]),
            emit(Key::X),
            SequenceOptions::default()
                .with_logical_abort_key(LogicalKeyValue::Character("x".into())),
        )
        .unwrap();
        d.register_sequence_pattern(
            seq(vec![c("k"), c("x"), p(Key::D)]),
            emit(Key::Y),
            SequenceOptions::default(),
        )
        .unwrap();
        d.process_event(&event(None, Some("k")));
        assert!(matches!(
            d.process_event(&event(Some(Key::Q), Some("x"))),
            MatchResult::Pending {
                steps_matched: 2,
                ..
            }
        ));
        assert_eq!(d.active_sequences.len(), 1);
        assert_eq!(emitted(d.process_event(&event(Some(Key::D), None))), Key::Y);
    }

    #[test]
    fn deferred_standalone_preserves_global_source_and_layer_declaration_rules() {
        use crate::device::DeviceContext;
        use crate::device::DeviceFilter;
        use crate::device::DeviceInfo;

        let prefix = event(Some(Key::K), Some("k"));
        for in_layer in [false, true] {
            let mut d = Dispatcher::new();
            let sequence = seq(vec![c("k"), p(Key::C)]);
            if in_layer {
                d.define_layer(
                    Layer::new("mode")
                        .bind_sequence_pattern(sequence, emit(Key::C), SequenceOptions::default())
                        .bind_pattern(
                            c("k"),
                            emit(Key::L),
                            BindingOptions::default().with_source("user"),
                        )
                        .bind_pattern(
                            p(Key::K),
                            emit(Key::F),
                            BindingOptions::default().with_source("default"),
                        )
                        .bind_pattern(
                            p(Key::K),
                            emit(Key::D),
                            BindingOptions::default()
                                .with_source("user")
                                .with_device(DeviceFilter::name_contains("pad")),
                        ),
                )
                .unwrap();
                d.push_layer("mode").unwrap();
            } else {
                d.register_sequence_pattern(sequence, emit(Key::C), SequenceOptions::default())
                    .unwrap();
                d.register_pattern(
                    p(Key::K),
                    emit(Key::F),
                    BindingOptions::default().with_source("default"),
                )
                .unwrap();
                d.register_pattern(
                    c("k"),
                    emit(Key::L),
                    BindingOptions::default().with_source("user"),
                )
                .unwrap();
            }
            let info = DeviceInfo::new("pad", 1, 2);
            let device = DeviceContext::new(1, &info);
            assert!(d.bindings_for_event_with_device(&prefix, &device).is_none());
            assert!(matches!(
                d.process_event_with_device(&prefix, &device),
                MatchResult::Pending { .. }
            ));
            let deadline = d.active_sequences[0].deadline;
            let timeout = d.check_sequence_timeouts(deadline).unwrap();
            // Layer: physical first declaration; global: logical User > physical Default.
            assert_eq!(
                emitted(d.match_pending_timeout(&timeout).unwrap()),
                if in_layer { Key::F } else { Key::L }
            );
        }
    }

    #[test]
    fn mixed_layer_removal_cancels_without_fallback_and_duplicate_activation_survives() {
        let mut d = Dispatcher::new();
        d.define_layer(
            Layer::new("mode")
                .bind_sequence_pattern(
                    seq(vec![c("k"), p(Key::C)]),
                    emit(Key::C),
                    SequenceOptions::default(),
                )
                .bind_pattern(
                    c("k"),
                    Action::PushLayer("other".into()),
                    BindingOptions::default(),
                ),
        )
        .unwrap();
        d.define_layer(Layer::new("other")).unwrap();
        d.push_layer("mode").unwrap();
        d.push_layer("mode").unwrap();
        d.process_event(&event(None, Some("k")));
        d.pop_layer().unwrap();
        assert_eq!(d.active_sequences.len(), 1);
        let deadline = d.active_sequences[0].deadline;
        d.pop_layer().unwrap();
        assert!(d.check_sequence_timeouts(deadline).is_none());
        assert!(d.active_layers().is_empty());
        assert!(d.pending_sequence().is_none());
        assert!(d.pending_standalone.is_none());
    }

    #[test]
    fn pending_snapshot_uses_domain_rank_and_completion_does_not_wait_for_longer_sequence() {
        let mut d = Dispatcher::new();
        install(
            &mut d,
            false,
            vec![
                (seq(vec![c("k"), c("c"), p(Key::D)]), Key::D),
                (seq(vec![p(Key::K), p(Key::C)]), Key::C),
            ],
        );
        assert!(matches!(
            d.process_event(&event(Some(Key::K), Some("k"))),
            MatchResult::Pending {
                steps_matched: 1,
                steps_remaining: 1
            }
        ));
        assert_eq!(d.pending_sequence().unwrap().steps_remaining, 1);
        assert_eq!(
            emitted(d.process_event(&event(Some(Key::C), Some("c")))),
            Key::C
        );
        assert!(d.pending_sequence().is_none());
    }

    fn pending_with_fallback() -> Dispatcher {
        let mut d = Dispatcher::new();
        install(
            &mut d,
            false,
            vec![
                (seq(vec![p(Key::K), p(Key::C)]), Key::C),
                (seq(vec![c("k"), p(Key::D)]), Key::D),
            ],
        );
        d.register(Key::K, emit(Key::F)).unwrap();
        d.register(Key::X, emit(Key::X)).unwrap();
        d.process_event(&event(Some(Key::K), Some("k")));
        assert_eq!(d.active_sequences.len(), 2);
        d
    }

    #[test]
    fn expired_sibling_plus_live_mismatch_retries_input_without_fallback() {
        let mut d = pending_with_fallback();
        d.active_sequences[0].deadline = Instant::now();
        d.active_sequences[1].deadline = Instant::now() + Duration::from_secs(60);
        assert_eq!(
            emitted(d.process_event(&event(Some(Key::X), Some("x")))),
            Key::X
        );
        assert!(d.pending_sequence().is_none());
        assert!(d.pending_standalone.is_none());
        assert!(d.pending_timeouts().is_empty());
    }

    #[test]
    fn timeout_poll_waits_for_last_candidate_and_fires_once_at_equality() {
        let mut d = pending_with_fallback();
        let now = Instant::now();
        d.active_sequences[0].deadline = now;
        d.active_sequences[1].deadline = now + Duration::from_millis(200);
        assert!(d.check_sequence_timeouts(now).is_none());
        assert_eq!(d.active_sequences.len(), 1);
        assert!(
            d.check_sequence_timeouts(now + Duration::from_millis(199))
                .is_none()
        );
        let timeout = d
            .check_sequence_timeouts(now + Duration::from_millis(200))
            .unwrap();
        assert_eq!(emitted(d.match_pending_timeout(&timeout).unwrap()), Key::F);
        assert!(
            d.check_sequence_timeouts(now + Duration::from_millis(201))
                .is_none()
        );
        assert!(d.pending_sequence().is_none());
    }

    #[test]
    fn all_expired_late_event_is_consumed_unless_timeout_is_polled_first() {
        for poll_first in [false, true] {
            let mut d = pending_with_fallback();
            for active in &mut d.active_sequences {
                active.deadline = Instant::now();
            }
            if poll_first {
                let timeouts = d.pending_timeouts();
                assert_eq!(timeouts.len(), 1);
                assert_eq!(
                    emitted(d.match_pending_timeout(&timeouts[0]).unwrap()),
                    Key::F
                );
            }
            assert_eq!(
                emitted(d.process_event(&event(Some(Key::X), None))),
                if poll_first { Key::X } else { Key::F }
            );
            assert!(d.pending_timeouts().is_empty());
        }
    }

    #[test]
    fn progress_refreshes_deadline_and_discards_initial_fallback() {
        let mut d = Dispatcher::new();
        d.register_sequence_pattern(
            seq(vec![p(Key::K), c("c"), p(Key::D)]),
            emit(Key::D),
            SequenceOptions::default().with_timeout(Duration::from_secs(60)),
        )
        .unwrap();
        d.register(Key::K, emit(Key::F)).unwrap();
        d.process_event(&event(Some(Key::K), None));
        let old_deadline = Instant::now() + Duration::from_secs(30);
        d.active_sequences[0].deadline = old_deadline;
        assert!(matches!(
            d.process_event(&event(Some(Key::Q), Some("c"))),
            MatchResult::Pending {
                steps_matched: 2,
                steps_remaining: 1
            }
        ));
        assert!(d.active_sequences[0].deadline > old_deadline);
        let deadline = d.active_sequences[0].deadline;
        assert!(d.check_sequence_timeouts(deadline).is_none());
        assert!(d.pending_sequence().is_none());
        assert!(d.pending_standalone.is_none());
    }

    #[test]
    fn cancellation_is_silent_idempotent_and_does_not_reset_other_state() {
        let mut d = pending_with_fallback();
        d.define_layer(Layer::new("mode")).unwrap();
        d.push_layer("mode").unwrap();
        d.register_with_options(
            Key::T,
            emit(Key::T),
            BindingOptions::default().with_debounce(Duration::from_secs(60)),
        )
        .unwrap();
        assert_eq!(emitted(d.process_event(&event(Some(Key::T), None))), Key::T);
        d.process_event(&event(Some(Key::K), Some("k")));
        d.register_tap_hold(
            Key::Q,
            emit(Key::A),
            emit(Key::B),
            crate::tap_hold::TapHoldOptions::new().with_threshold(Duration::from_secs(60)),
        )
        .unwrap();
        d.process_event(&event(Some(Key::Q), None));
        // Q enrollment does not advance/cancel pending sequences.
        assert!(d.pending_sequence().is_some());
        d.cancel_pending_sequence();
        d.cancel_pending_sequence();
        assert!(d.pending_sequence().is_none());
        assert!(d.pending_standalone.is_none());
        assert_eq!(d.active_layers().len(), 1);
        assert!(d.is_registered(Hotkey::new(Key::K)));
        let mut release = event(Some(Key::Q), None);
        release.transition = KeyTransition::Release;
        assert_eq!(emitted(d.process_event(&release)), Key::A);
        assert!(matches!(
            d.process_event(&event(Some(Key::T), None)),
            MatchResult::Throttled { .. }
        ));
        assert!(d.pending_timeouts().is_empty());
        assert!(matches!(
            d.process_event(&event(Some(Key::K), Some("k"))),
            MatchResult::Pending { .. }
        ));
    }

    #[test]
    fn physical_registration_duplicates_and_selective_removal_remain_consistent() {
        let mut d = Dispatcher::new();
        let a = d.register_sequence("K, C", emit(Key::C)).unwrap();
        assert!(
            d.register_sequence_pattern(
                seq(vec![p(Key::K), p(Key::C)]),
                emit(Key::X),
                SequenceOptions::default()
            )
            .is_err()
        );
        let b = d
            .register_sequence_pattern(
                seq(vec![c("k"), p(Key::D)]),
                emit(Key::D),
                SequenceOptions::default(),
            )
            .unwrap();
        let fallback = d.register(Key::K, emit(Key::F)).unwrap();
        d.process_event(&event(Some(Key::K), Some("k")));
        d.unregister(a);
        assert_eq!(d.active_sequences.len(), 1);
        d.unregister(fallback);
        assert!(d.pending_standalone.is_none());
        assert_eq!(emitted(d.process_event(&event(Some(Key::D), None))), Key::D);
        d.process_event(&event(None, Some("k")));
        d.unregister(b);
        assert!(d.pending_sequence().is_none());
        assert!(d.next_timeout_deadline().is_none());
    }
}
