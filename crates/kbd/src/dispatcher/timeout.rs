use std::time::Duration;
use std::time::Instant;

use super::Dispatcher;
use super::MatchResult;
use super::sequence::StandaloneMatch;
use crate::binding::BindingId;
use crate::key::Key;
use crate::policy::KeyPropagation;
use crate::policy::RepeatPolicy;

/// A timeout that fired but hasn't been matched to an action yet.
///
/// This is the first half of a two-phase pattern: collect pending
/// timeouts first (mutating state), then match each to a
/// [`MatchResult`] via [`Dispatcher::match_pending_timeout`].
///
/// This type is opaque — callers receive it from
/// [`pending_timeouts`](Dispatcher::pending_timeouts)
/// and pass it back to [`match_pending_timeout`](Dispatcher::match_pending_timeout).
pub struct PendingTimeout {
    pub(super) kind: TimeoutKind,
}

impl PendingTimeout {
    /// Returns the key associated with this timeout, if it's a tap-hold hold resolution.
    ///
    /// The engine uses this to update the press cache after a hold resolves
    /// (by timeout or interrupt), enabling correct repeat and release handling.
    #[must_use]
    pub fn tap_hold_key(&self) -> Option<Key> {
        match &self.kind {
            TimeoutKind::TapHoldHold { key, .. } => Some(*key),
            TimeoutKind::Standalone(_) => None,
        }
    }
}

pub(super) enum TimeoutKind {
    Standalone(StandaloneMatch),
    TapHoldHold { key: Key, binding_id: BindingId },
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

    /// Process deferred state transitions and return any that fired.
    ///
    /// Handles layer auto-pop, sequence step timeouts, and tap-hold hold
    /// resolution (both timeout-based and interrupt-based). Returns
    /// [`PendingTimeout`] values that can be matched to actions via
    /// [`match_pending_timeout`](Self::match_pending_timeout).
    pub fn pending_timeouts(&mut self) -> Vec<PendingTimeout> {
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

        let mut pending: Vec<PendingTimeout> = self
            .tap_hold
            .check_timeouts(now)
            .into_iter()
            .chain(self.tap_hold.drain_resolved_holds())
            .map(|(key, binding_id)| PendingTimeout {
                kind: TimeoutKind::TapHoldHold { key, binding_id },
            })
            .collect();

        if let Some(p) = self.check_sequence_timeouts(now) {
            pending.push(p);
        }

        pending
    }

    /// Match a [`PendingTimeout`] to its action, returning a [`MatchResult`].
    ///
    /// This is the second step of the two-phase timeout pattern: first
    /// collect pending timeouts (which mutate state), then match each to
    /// an action reference.
    #[must_use]
    pub fn match_pending_timeout(&self, pending: &PendingTimeout) -> Option<MatchResult<'_>> {
        match &pending.kind {
            TimeoutKind::Standalone(standalone) => Some(MatchResult::Matched {
                action: self.resolve_binding(&standalone.binding_ref),
                propagation: standalone.propagation,
                repeat_policy: standalone.repeat_policy,
            }),
            TimeoutKind::TapHoldHold { binding_id, .. } => self
                .tap_hold
                .hold_action(*binding_id)
                .map(|action| MatchResult::Matched {
                    action,
                    propagation: KeyPropagation::Stop,
                    repeat_policy: RepeatPolicy::Suppress,
                }),
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
