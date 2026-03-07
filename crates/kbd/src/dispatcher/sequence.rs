use std::time::Duration;
use std::time::Instant;

use super::Dispatcher;
use super::InternalOutcome;
use super::MatchResult;
use super::MatchedBindingRef;
use super::layers::LayerEffect;
use crate::binding::BindingId;
use crate::binding::KeyPropagation;
use crate::hotkey::Hotkey;
use crate::hotkey::HotkeySequence;
use crate::layer::LayerName;
use crate::policy::RepeatPolicy;
use crate::sequence::PendingSequenceInfo;
use crate::sequence::SequenceOptions;

pub(super) struct RegisteredSequenceBinding {
    pub(super) id: BindingId,
    pub(super) sequence: HotkeySequence,
    pub(super) action: crate::action::Action,
    pub(super) propagation: KeyPropagation,
    pub(super) options: SequenceOptions,
}

impl RegisteredSequenceBinding {
    pub(super) fn new(
        id: BindingId,
        sequence: HotkeySequence,
        action: crate::action::Action,
        options: SequenceOptions,
    ) -> Self {
        Self {
            id,
            sequence,
            action,
            propagation: KeyPropagation::Stop,
            options,
        }
    }
}

#[derive(Clone)]
pub(super) enum SequenceBindingRef {
    Global(BindingId),
    Layer { name: LayerName, index: usize },
}

pub(super) struct ActiveSequence {
    pub(super) binding_ref: SequenceBindingRef,
    pub(super) next_step_index: usize,
    pub(super) deadline: Instant,
    pub(super) priority: usize,
}

pub(super) struct PendingStandalone {
    pub(super) binding_ref: MatchedBindingRef,
    pub(super) propagation: KeyPropagation,
    pub(super) layer_effect: LayerEffect,
    pub(super) repeat_policy: RepeatPolicy,
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
    pub(super) fn match_active_sequences(&mut self, hotkey: &Hotkey) -> Option<InternalOutcome> {
        if self.active_sequences.is_empty() {
            return None;
        }

        let now = Instant::now();
        let mut survivors = Vec::new();
        let mut completed: Vec<(usize, SequenceBindingRef)> = Vec::new();
        let mut expired = false;
        let mut aborted = false;
        let active_sequences = std::mem::take(&mut self.active_sequences);

        for mut active in active_sequences {
            if active.deadline <= now {
                expired = true;
                continue;
            }

            if self.sequence_step_matches(&active.binding_ref, active.next_step_index, hotkey) {
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

            if self.sequence_options(&active.binding_ref).abort_key() == hotkey.key() {
                aborted = true;
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
                return Some(InternalOutcome::Pending {
                    steps_matched: pending.steps_matched,
                    steps_remaining: pending.steps_remaining,
                });
            }
            return Some(InternalOutcome::NoMatch);
        }

        self.active_sequences.clear();

        if aborted {
            self.pending_standalone = None;
            return Some(InternalOutcome::NoMatch);
        }

        if expired && let Some(standalone) = self.pending_standalone.take() {
            return Some(InternalOutcome::Matched {
                binding_ref: standalone.binding_ref,
                layer_effect: standalone.layer_effect,
                propagation: standalone.propagation,
                repeat_policy: standalone.repeat_policy,
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
    ) -> Option<InternalOutcome> {
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
                    return Some(InternalOutcome::Matched {
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
            return Some(InternalOutcome::Pending {
                steps_matched: pending.steps_matched,
                steps_remaining: pending.steps_remaining,
            });
        }

        Some(InternalOutcome::NoMatch)
    }

    pub(super) fn pending_standalone_from_match(
        &self,
        binding_match: Option<(MatchedBindingRef, KeyPropagation, RepeatPolicy)>,
    ) -> Option<PendingStandalone> {
        binding_match.map(
            |(binding_ref, propagation, repeat_policy)| PendingStandalone {
                layer_effect: LayerEffect::from_action(self.resolve_binding(&binding_ref)),
                binding_ref,
                propagation,
                repeat_policy,
            },
        )
    }

    pub(super) fn check_sequence_timeouts(&mut self, now: Instant) -> Vec<MatchResult<'_>> {
        if self.active_sequences.is_empty() {
            return Vec::new();
        }

        let before = self.active_sequences.len();
        self.active_sequences.retain(|active| active.deadline > now);
        let expired = before.saturating_sub(self.active_sequences.len());

        if expired > 0 && self.active_sequences.is_empty() {
            if let Some(standalone) = self.pending_standalone.take() {
                self.apply_layer_effect(&standalone.layer_effect);
                let action = self.resolve_binding(&standalone.binding_ref);
                return vec![MatchResult::Matched {
                    action,
                    propagation: standalone.propagation,
                    repeat_policy: standalone.repeat_policy,
                }];
            }

            self.pending_standalone = None;
        }

        Vec::new()
    }

    fn sequence_step_count(&self, binding_ref: &SequenceBindingRef) -> usize {
        match binding_ref {
            SequenceBindingRef::Global(id) => {
                self.sequence_bindings_by_id[id].sequence.steps().len()
            }
            SequenceBindingRef::Layer { name, index } => self.layers[name].sequence_bindings
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
        hotkey: &Hotkey,
    ) -> bool {
        match binding_ref {
            SequenceBindingRef::Global(id) => self.sequence_bindings_by_id[id]
                .sequence
                .steps()
                .get(step_index)
                .is_some_and(|step| step == hotkey),
            SequenceBindingRef::Layer { name, index } => self.layers[name].sequence_bindings
                [*index]
                .sequence
                .steps()
                .get(step_index)
                .is_some_and(|step| step == hotkey),
        }
    }

    fn sequence_options(&self, binding_ref: &SequenceBindingRef) -> SequenceOptions {
        match binding_ref {
            SequenceBindingRef::Global(id) => self.sequence_bindings_by_id[id].options,
            SequenceBindingRef::Layer { name, index } => {
                self.layers[name].sequence_bindings[*index].options
            }
        }
    }

    fn matched_outcome_for_sequence(&self, sequence_ref: SequenceBindingRef) -> InternalOutcome {
        match sequence_ref {
            SequenceBindingRef::Global(id) => {
                let binding = &self.sequence_bindings_by_id[&id];
                InternalOutcome::Matched {
                    binding_ref: MatchedBindingRef::SequenceGlobal(id),
                    layer_effect: LayerEffect::from_action(&binding.action),
                    propagation: binding.propagation,
                    repeat_policy: RepeatPolicy::default(),
                }
            }
            SequenceBindingRef::Layer { name, index } => {
                let binding = &self.layers[&name].sequence_bindings[index];
                InternalOutcome::Matched {
                    binding_ref: MatchedBindingRef::SequenceLayer { name, index },
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
            .map(|active| {
                let total = self.sequence_step_count(&active.binding_ref);
                PendingSequenceInfo {
                    steps_matched: active.next_step_index,
                    steps_remaining: total.saturating_sub(active.next_step_index),
                }
            })
            .max_by_key(|pending| pending.steps_matched)
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
                pending.binding_ref,
                MatchedBindingRef::Layer { ref name, .. }
                    | MatchedBindingRef::SequenceLayer { ref name, .. }
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
            &Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(first, MatchResult::Pending { .. }));

        let second = dispatcher.process(
            &Hotkey::new(Key::C).modifier(Modifier::Ctrl),
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
            &Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(first, MatchResult::Pending { .. }));

        std::thread::sleep(Duration::from_millis(20));
        for timeout_result in dispatcher.check_timeouts_with_results() {
            execute_callback(&timeout_result);
        }

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
            &Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(first, MatchResult::Pending { .. }));

        let wrong = dispatcher.process(
            &Hotkey::new(Key::X).modifier(Modifier::Ctrl),
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
            &Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(first, MatchResult::Pending { .. }));

        let aborted = dispatcher.process(&Hotkey::new(Key::ESCAPE), KeyTransition::Press);
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
            &Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(first, MatchResult::Pending { .. }));

        let second = dispatcher.process(&Hotkey::new(Key::ESCAPE), KeyTransition::Press);
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
            &Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(pending, MatchResult::Pending { .. }));

        std::thread::sleep(Duration::from_millis(20));
        for timeout_result in dispatcher.check_timeouts_with_results() {
            execute_callback(&timeout_result);
        }

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
            &Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(first, MatchResult::Pending { .. }));

        let second = dispatcher.process(
            &Hotkey::new(Key::S).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(second, MatchResult::Pending { .. }));

        std::thread::sleep(Duration::from_millis(20));
        for timeout_result in dispatcher.check_timeouts_with_results() {
            execute_callback(&timeout_result);
        }

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
            &Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(first, MatchResult::Pending { .. }));
        let second = dispatcher.process(
            &Hotkey::new(Key::S).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(second, MatchResult::Pending { .. }));

        let third = dispatcher.process(
            &Hotkey::new(Key::D).modifier(Modifier::Ctrl),
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
            &Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(first, MatchResult::Pending { .. }));

        std::thread::sleep(Duration::from_millis(20));
        dispatcher.check_timeouts();

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
            &Hotkey::new(Key::K).modifier(Modifier::Ctrl),
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
            &Hotkey::new(Key::C).modifier(Modifier::Ctrl),
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
            &Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(first, MatchResult::Pending { .. }));

        dispatcher.unregister(standalone_id);

        std::thread::sleep(Duration::from_millis(20));
        let timeout_results = dispatcher.check_timeouts_with_results();
        assert!(timeout_results.is_empty());
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
            &Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(first, MatchResult::Pending { .. }));

        std::thread::sleep(Duration::from_millis(20));
        for timeout_result in dispatcher.check_timeouts_with_results() {
            execute_callback(&timeout_result);
        }

        let h = dispatcher.process(&Hotkey::new(Key::H), KeyTransition::Press);
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
            &Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(first, MatchResult::Pending { .. }));

        let pending = dispatcher.pending_sequence().expect("pending sequence");
        assert_eq!(pending.steps_matched, 1);
        assert_eq!(pending.steps_remaining, 1);
    }
}
