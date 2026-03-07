use std::collections::HashMap;
use std::collections::VecDeque;
use std::time::Duration;
use std::time::Instant;

use super::Dispatcher;
use super::InternalOutcome;
use super::MatchedBindingRef;
use crate::binding::BindingId;
use crate::binding::BindingOptions;
use crate::layer::LayerName;

/// Tracks per-binding timing state for debounce and rate limiting.
#[derive(Default)]
pub(crate) struct ThrottleTracker {
    state: HashMap<ThrottleKey, ThrottleState>,
}

/// Unique key identifying a binding for throttle tracking.
///
/// Mirrors [`MatchedBindingRef`] but derives Hash/Eq for use as a
/// `HashMap` key.
#[derive(Clone, Hash, PartialEq, Eq)]
enum ThrottleKey {
    Global(BindingId),
    Layer { name: LayerName, index: usize },
    SequenceGlobal(BindingId),
    SequenceLayer { name: LayerName, index: usize },
}

impl From<&MatchedBindingRef> for ThrottleKey {
    fn from(binding_ref: &MatchedBindingRef) -> Self {
        match binding_ref {
            MatchedBindingRef::Global(id) => ThrottleKey::Global(*id),
            MatchedBindingRef::Layer { name, index } => ThrottleKey::Layer {
                name: name.clone(),
                index: *index,
            },
            MatchedBindingRef::SequenceGlobal(id) => ThrottleKey::SequenceGlobal(*id),
            MatchedBindingRef::SequenceLayer { name, index } => ThrottleKey::SequenceLayer {
                name: name.clone(),
                index: *index,
            },
        }
    }
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
    fn record_fire(&mut self, key: ThrottleKey, now: Instant, has_rate_limit: bool) {
        let state = self.state.entry(key).or_default();
        state.last_fire = Some(now);
        if has_rate_limit {
            state.recent_fires.push_back(now);
        }
    }

    /// Check if a binding should be throttled by debounce.
    ///
    /// Returns `true` if the binding should be suppressed.
    fn is_debounced(&self, key: &ThrottleKey, debounce: Duration, now: Instant) -> bool {
        self.state
            .get(key)
            .and_then(|s| s.last_fire)
            .is_some_and(|last| now.duration_since(last) < debounce)
    }

    /// Check if a binding should be throttled by rate limit.
    ///
    /// Prunes expired entries from the window and returns `true` if
    /// the count within the window meets or exceeds `max_count`.
    fn is_rate_limited(
        &mut self,
        key: &ThrottleKey,
        max_count: u32,
        window: Duration,
        now: Instant,
    ) -> bool {
        let Some(state) = self.state.get_mut(key) else {
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

    /// Remove throttle state for a specific global binding (on unregister).
    pub(crate) fn remove_global(&mut self, id: BindingId) {
        self.state.remove(&ThrottleKey::Global(id));
    }
}

impl Dispatcher {
    /// Check if a matched binding should be throttled by debounce or
    /// rate limit. If throttled, converts the outcome to `Throttled`.
    /// If not, records the fire time and returns the outcome unchanged.
    pub(super) fn check_throttle(&mut self, outcome: InternalOutcome) -> InternalOutcome {
        let InternalOutcome::Matched {
            ref binding_ref,
            propagation,
            ..
        } = outcome
        else {
            return outcome;
        };

        // Copy throttle options out before any mutable borrows on self.
        let options = self.binding_options(binding_ref);
        let debounce = options.debounce();
        let rate_limit = options.rate_limit().copied();

        // No throttle policy configured — skip all tracking.
        if debounce.is_none() && rate_limit.is_none() {
            return outcome;
        }

        let now = Instant::now();
        let throttle_key = ThrottleKey::from(binding_ref);

        // Check debounce
        if let Some(debounce) = debounce {
            if self
                .throttle_tracker
                .is_debounced(&throttle_key, debounce, now)
            {
                return InternalOutcome::Throttled { propagation };
            }
        }

        // Check rate limit
        if let Some(rate_limit) = rate_limit {
            if self.throttle_tracker.is_rate_limited(
                &throttle_key,
                rate_limit.max_count(),
                rate_limit.window(),
                now,
            ) {
                return InternalOutcome::Throttled { propagation };
            }
        }

        // Not throttled — record this fire
        self.throttle_tracker
            .record_fire(throttle_key, now, rate_limit.is_some());

        outcome
    }

    /// Look up the binding options for a given binding reference.
    ///
    /// Sequence bindings don't carry `BindingOptions`, so they return
    /// the default (no debounce, no rate limit, suppress repeats).
    fn binding_options(&self, binding_ref: &MatchedBindingRef) -> &BindingOptions {
        // Sequence bindings use SequenceOptions (timeout, abort_key) rather
        // than BindingOptions. Since they don't support debounce/rate-limit,
        // return a static default.
        static DEFAULT_OPTIONS: BindingOptions = BindingOptions::DEFAULT;

        match binding_ref {
            MatchedBindingRef::Global(id) => self.bindings_by_id[id].options(),
            MatchedBindingRef::Layer { name, index } => &self.layers[name].bindings[*index].options,
            MatchedBindingRef::SequenceGlobal(_) | MatchedBindingRef::SequenceLayer { .. } => {
                &DEFAULT_OPTIONS
            }
        }
    }
}
