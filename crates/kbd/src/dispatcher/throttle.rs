use std::collections::HashMap;
use std::collections::VecDeque;
use std::time::Duration;
use std::time::Instant;

use super::BindingMatch;
use super::Dispatcher;
use super::MatchedBindingRef;
use crate::binding::BindingId;
use crate::binding::BindingOptions;

/// Tracks per-binding timing state for debounce and rate limiting.
#[derive(Default)]
pub(super) struct ThrottleTracker {
    state: HashMap<BindingId, ThrottleState>,
}

#[derive(Default)]
struct ThrottleState {
    /// When the binding last successfully fired (not throttled).
    last_fire: Option<Instant>,
    /// Timestamps of recent fires for rate limiting (sliding window).
    recent_fires: VecDeque<Instant>,
}

impl ThrottleTracker {
    /// Record that a binding has fired at the given time.
    ///
    /// `has_rate_limit` controls whether the fire is also pushed into
    /// `recent_fires` for sliding-window rate limiting. Without it,
    /// only `last_fire` is updated (for debounce tracking).
    fn record_fire(&mut self, id: BindingId, now: Instant, has_rate_limit: bool) {
        let state = self.state.entry(id).or_default();
        state.last_fire = Some(now);
        if has_rate_limit {
            state.recent_fires.push_back(now);
        }
    }

    /// Check if a binding should be throttled by debounce.
    ///
    /// Returns `true` if the binding should be suppressed.
    fn is_debounced(&self, id: BindingId, debounce: Duration, now: Instant) -> bool {
        self.state
            .get(&id)
            .and_then(|s| s.last_fire)
            .is_some_and(|last| now.duration_since(last) < debounce)
    }

    /// Check if a binding should be throttled by rate limit.
    ///
    /// Prunes expired entries from the window and returns `true` if
    /// the count within the window meets or exceeds `max_count`.
    fn is_rate_limited(
        &mut self,
        id: BindingId,
        max_count: u32,
        window: Duration,
        now: Instant,
    ) -> bool {
        let Some(state) = self.state.get_mut(&id) else {
            return false;
        };

        // Remove entries outside the window
        while state
            .recent_fires
            .front()
            .is_some_and(|&t| now.duration_since(t) >= window)
        {
            state.recent_fires.pop_front();
        }

        state.recent_fires.len() >= max_count as usize
    }

    /// Remove throttle state for a binding (on unregister or layer removal).
    pub(super) fn remove(&mut self, id: BindingId) {
        self.state.remove(&id);
    }
}

impl Dispatcher {
    /// Check if a matched binding should be throttled by debounce or
    /// rate limit. If throttled, converts the outcome to `Throttled`.
    /// If not, records the fire time and returns the outcome unchanged.
    pub(super) fn check_throttle(&mut self, outcome: BindingMatch) -> BindingMatch {
        let BindingMatch::Matched {
            ref binding_ref,
            propagation,
            ..
        } = outcome
        else {
            return outcome;
        };

        // Look up options for the matched binding. Sequence bindings
        // don't carry BindingOptions, so they fall back to the default.
        let options = match binding_ref {
            MatchedBindingRef::Global(id) => self.bindings_by_id[id].options(),
            MatchedBindingRef::Layer { name, index, .. } => {
                self.layers[name].bindings[*index].options()
            }
            MatchedBindingRef::SequenceGlobal(_) | MatchedBindingRef::SequenceLayer { .. } => {
                &BindingOptions::default()
            }
        };
        let debounce = options.debounce();
        let rate_limit = options.rate_limit();

        // No throttle policy configured — skip all tracking.
        if debounce.is_none() && rate_limit.is_none() {
            return outcome;
        }

        let now = Instant::now();
        let id = binding_ref.binding_id();

        // Check debounce
        if let Some(debounce) = debounce {
            if self.throttle_tracker.is_debounced(id, debounce, now) {
                return BindingMatch::Throttled { propagation };
            }
        }

        // Check rate limit
        if let Some(rate_limit) = rate_limit {
            if self.throttle_tracker.is_rate_limited(
                id,
                rate_limit.max_count(),
                rate_limit.window(),
                now,
            ) {
                return BindingMatch::Throttled { propagation };
            }
        }

        // Not throttled — record this fire
        self.throttle_tracker
            .record_fire(id, now, rate_limit.is_some());

        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_fire_is_not_debounced() {
        let tracker = ThrottleTracker::default();
        let id = BindingId::new();
        let now = Instant::now();
        assert!(!tracker.is_debounced(id, Duration::from_millis(100), now));
    }

    #[test]
    fn fire_within_debounce_window_is_suppressed() {
        let mut tracker = ThrottleTracker::default();
        let id = BindingId::new();
        let t0 = Instant::now();
        tracker.record_fire(id, t0, false);

        assert!(tracker.is_debounced(id, Duration::from_millis(100), t0));
    }

    #[test]
    fn fire_after_debounce_window_is_allowed() {
        let mut tracker = ThrottleTracker::default();
        let id = BindingId::new();
        let t0 = Instant::now();
        tracker.record_fire(id, t0, false);

        let t1 = t0 + Duration::from_millis(101);
        assert!(!tracker.is_debounced(id, Duration::from_millis(100), t1));
    }

    #[test]
    fn debounce_is_per_binding() {
        let mut tracker = ThrottleTracker::default();
        let id_a = BindingId::new();
        let id_b = BindingId::new();
        let t0 = Instant::now();
        tracker.record_fire(id_a, t0, false);

        assert!(!tracker.is_debounced(id_b, Duration::from_millis(100), t0));
    }

    #[test]
    fn first_fire_is_not_rate_limited() {
        let mut tracker = ThrottleTracker::default();
        let id = BindingId::new();
        let now = Instant::now();
        assert!(!tracker.is_rate_limited(id, 3, Duration::from_secs(1), now));
    }

    #[test]
    fn fires_up_to_max_are_allowed() {
        let mut tracker = ThrottleTracker::default();
        let id = BindingId::new();
        let t0 = Instant::now();

        for i in 0..3 {
            let t = t0 + Duration::from_millis(i * 10);
            assert!(!tracker.is_rate_limited(id, 3, Duration::from_secs(1), t));
            tracker.record_fire(id, t, true);
        }

        let t3 = t0 + Duration::from_millis(30);
        assert!(tracker.is_rate_limited(id, 3, Duration::from_secs(1), t3));
    }

    #[test]
    fn rate_limit_resets_after_window() {
        let mut tracker = ThrottleTracker::default();
        let id = BindingId::new();
        let t0 = Instant::now();

        for i in 0..3 {
            let t = t0 + Duration::from_millis(i * 10);
            tracker.record_fire(id, t, true);
        }

        let t_after = t0 + Duration::from_secs(1);
        assert!(!tracker.is_rate_limited(id, 3, Duration::from_secs(1), t_after));
    }

    #[test]
    fn remove_clears_state() {
        let id = BindingId::new();
        let mut tracker = ThrottleTracker::default();
        let t0 = Instant::now();
        tracker.record_fire(id, t0, false);

        assert!(tracker.is_debounced(id, Duration::from_millis(100), t0));

        tracker.remove(id);

        assert!(!tracker.is_debounced(id, Duration::from_millis(100), t0));
    }
}
