use std::time::Duration;
use std::time::Instant;

use super::Dispatcher;
use super::MatchResult;
use super::MatchedBindingRef;
use crate::binding::BindingId;
use crate::policy::KeyPropagation;
use crate::policy::RepeatPolicy;

/// Borrow-free result of a timeout check, used to decouple state mutation
/// from action resolution. Same pattern as `TapHoldDecision` in `process()`.
///
/// The caller collects all resolutions first (mutating state), then resolves
/// each to a [`MatchResult`] via [`Dispatcher::resolve_timeout`].
///
/// This type is opaque — callers receive it from
/// [`check_timeouts`](Dispatcher::check_timeouts)
/// and pass it back to [`resolve_timeout`](Dispatcher::resolve_timeout).
pub struct TimeoutResolution {
    kind: TimeoutKind,
}

enum TimeoutKind {
    Standalone {
        binding_ref: MatchedBindingRef,
        propagation: KeyPropagation,
        repeat_policy: RepeatPolicy,
    },
    TapHoldHold {
        binding_id: BindingId,
    },
}

impl TimeoutResolution {
    pub(super) fn standalone(
        binding_ref: MatchedBindingRef,
        propagation: KeyPropagation,
        repeat_policy: RepeatPolicy,
    ) -> Self {
        Self {
            kind: TimeoutKind::Standalone {
                binding_ref,
                propagation,
                repeat_policy,
            },
        }
    }

    pub(super) fn tap_hold_hold(binding_id: BindingId) -> Self {
        Self {
            kind: TimeoutKind::TapHoldHold { binding_id },
        }
    }
}

impl Dispatcher {
    /// Return the nearest layer, sequence, or tap-hold timeout deadline, if any.
    #[must_use]
    pub fn next_timeout_deadline(&self) -> Option<Duration> {
        let now = Instant::now();
        let mut min_remaining: Option<Duration> = None;

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

        for active in &self.active_sequences {
            let remaining = active.deadline.saturating_duration_since(now);
            min_remaining = Some(match min_remaining {
                Some(current) => std::cmp::min(current, remaining),
                None => remaining,
            });
        }

        if let Some(tap_hold_remaining) = self.tap_hold.next_deadline(now) {
            min_remaining = Some(match min_remaining {
                Some(current) => std::cmp::min(current, tap_hold_remaining),
                None => tap_hold_remaining,
            });
        }

        min_remaining
    }

    /// Check all timeout-driven state transitions and return resolutions.
    ///
    /// Handles layer auto-pop, sequence step timeouts, and tap-hold hold
    /// resolution. Returns borrow-free [`TimeoutResolution`] values that
    /// can be resolved to [`MatchResult`] via [`resolve_timeout`](Self::resolve_timeout).
    pub fn check_timeouts(&mut self) -> Vec<TimeoutResolution> {
        let now = Instant::now();

        let mut timed_out_layers = Vec::new();
        self.layer_stack.retain(|entry| {
            let keep = if let Some(timeout) = &entry.timeout {
                now.duration_since(timeout.last_activity) < timeout.duration
            } else {
                true
            };
            if !keep {
                timed_out_layers.push(entry.name.clone());
            }
            keep
        });
        for layer_name in timed_out_layers {
            self.clear_sequences_for_layer_if_inactive(&layer_name);
        }

        let mut resolutions = self.check_sequence_timeouts(now);

        for id in self.tap_hold.check_timeouts(now) {
            resolutions.push(TimeoutResolution::tap_hold_hold(id));
        }

        resolutions
    }

    /// Resolve a [`TimeoutResolution`] to a [`MatchResult`].
    ///
    /// This is the second step of the two-phase timeout pattern: first
    /// collect resolutions (which mutate state), then resolve each to
    /// an action reference.
    #[must_use]
    pub fn resolve_timeout(&self, resolution: &TimeoutResolution) -> Option<MatchResult<'_>> {
        match &resolution.kind {
            TimeoutKind::Standalone {
                binding_ref,
                propagation,
                repeat_policy,
            } => Some(MatchResult::Matched {
                action: self.resolve_binding(binding_ref),
                propagation: *propagation,
                repeat_policy: *repeat_policy,
            }),
            TimeoutKind::TapHoldHold { binding_id } => {
                self.tap_hold
                    .hold_action(*binding_id)
                    .map(|action| MatchResult::Matched {
                        action,
                        propagation: KeyPropagation::Stop,
                        repeat_policy: RepeatPolicy::Suppress,
                    })
            }
        }
    }

    /// Reset all layer inactivity timeouts to `now`.
    ///
    /// Called on every non-ignored key event so that layers remain alive
    /// while the user is actively typing.
    pub(super) fn reset_layer_timeouts(&mut self) {
        let now = Instant::now();
        for entry in &mut self.layer_stack {
            if let Some(timeout) = &mut entry.timeout {
                timeout.last_activity = now;
            }
        }
    }

    /// Tick the topmost oneshot layer, popping it when its count reaches zero.
    ///
    /// Oneshot layers are event-driven (not time-based): each qualifying
    /// keypress decrements the counter of the topmost oneshot layer. Only one
    /// oneshot layer is ticked per event.
    pub(super) fn tick_oneshot_layers(&mut self) {
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use super::super::Dispatcher;
    use super::super::MatchResult;
    use crate::action::Action;
    use crate::hotkey::Hotkey;
    use crate::key::Key;
    use crate::key_state::KeyTransition;
    use crate::layer::Layer;
    use crate::layer::LayerName;

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
            .unwrap()
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
        dispatcher.process(Hotkey::new(Key::F1), KeyTransition::Press);

        // First keypress in the oneshot layer — should match and then pop
        let result = dispatcher.process(Hotkey::new(Key::H), KeyTransition::Press);
        if let MatchResult::Matched {
            action: Action::Callback(cb),
            ..
        } = result
        {
            cb();
        }
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        // Second press → layer should be gone now
        let result = dispatcher.process(Hotkey::new(Key::H), KeyTransition::Press);
        assert!(matches!(result, MatchResult::NoMatch));
    }
}
