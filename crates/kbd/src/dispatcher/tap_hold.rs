//! Tap-hold state machine for the dispatcher.
//!
//! Tracks active tap-hold keys — keys that have been pressed but not yet
//! resolved as tap or hold. Resolution happens on:
//! - Release before threshold → tap
//! - Timeout past threshold → hold (via `check_timeouts`)
//! - Interrupting keypress → hold (keyd model)

use std::collections::HashMap;
use std::time::Duration;
use std::time::Instant;

use crate::action::Action;
use crate::binding::BindingId;
use crate::key::Key;
use crate::tap_hold::TapHoldOptions;

/// A registered tap-hold binding.
pub(crate) struct TapHoldBinding {
    pub(crate) id: BindingId,
    pub(crate) key: Key,
    pub(crate) tap_action: Action,
    pub(crate) hold_action: Action,
    pub(crate) options: TapHoldOptions,
}

/// Whether a tap-hold key has been resolved as a hold yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HoldResolution {
    /// Still waiting — could be tap or hold.
    Pending,
    /// Resolved as hold (by timeout or interrupt).
    Resolved,
}

/// An active (pressed) tap-hold key being tracked.
struct ActiveTapHold {
    pressed_at: Instant,
    resolution: HoldResolution,
    binding_id: BindingId,
}

/// State machine for tap-hold processing within the dispatcher.
#[derive(Default)]
pub(crate) struct TapHoldState {
    /// Registered tap-hold bindings, keyed by trigger key.
    bindings: HashMap<Key, TapHoldBinding>,
    /// Currently active (pressed) tap-hold keys.
    active: HashMap<Key, ActiveTapHold>,
    /// Holds resolved by interrupt, buffered for the engine to drain
    /// via the `pending_timeouts` pipeline.
    resolved_holds: Vec<(Key, BindingId)>,
}

/// What the tap-hold state machine decided about an event.
pub(crate) enum TapHoldOutcome {
    /// The key press was consumed (buffered for tap-hold).
    Consumed,
    /// Tap resolved — return the tap action.
    TapResolved { binding_id: BindingId },
    /// Repeat event consumed for an active tap-hold key.
    RepeatConsumed,
    /// Not a tap-hold key — pass through to normal matching.
    PassThrough,
}

impl TapHoldState {
    /// Register a tap-hold binding.
    pub(crate) fn register(&mut self, binding: TapHoldBinding) {
        self.bindings.insert(binding.key, binding);
    }

    /// Unregister a tap-hold binding by ID.
    pub(crate) fn unregister(&mut self, id: BindingId) {
        self.bindings.retain(|_, b| b.id != id);
        self.active.retain(|_, a| a.binding_id != id);
    }

    /// Returns `true` if any tap-hold bindings exist or any keys are
    /// actively being tracked. Used for fast-path skipping in `process()`.
    #[inline]
    pub(crate) fn has_state(&self) -> bool {
        !self.bindings.is_empty() || !self.active.is_empty()
    }

    /// Check if a key has a tap-hold binding registered.
    pub(crate) fn is_registered(&self, key: Key) -> bool {
        self.bindings.contains_key(&key)
    }

    /// Process a key press event for tap-hold.
    pub(crate) fn on_press(&mut self, key: Key, now: Instant) -> TapHoldOutcome {
        // Resolve any pending tap-holds that get interrupted by this press.
        // Resolved holds are buffered internally and drained via
        // `drain_resolved_holds` in the pending_timeouts pipeline.
        self.resolve_pending_for_interrupt(key);

        if let Some(binding) = self.bindings.get(&key) {
            let binding_id = binding.id;

            // If this key was already active (e.g., re-press without release),
            // clean up the old state.
            self.active.remove(&key);

            self.active.insert(
                key,
                ActiveTapHold {
                    pressed_at: now,
                    resolution: HoldResolution::Pending,
                    binding_id,
                },
            );

            TapHoldOutcome::Consumed
        } else {
            TapHoldOutcome::PassThrough
        }
    }

    /// Process a key release event for tap-hold.
    pub(crate) fn on_release(&mut self, key: Key) -> TapHoldOutcome {
        let Some(active) = self.active.remove(&key) else {
            return TapHoldOutcome::PassThrough;
        };

        match active.resolution {
            HoldResolution::Pending => TapHoldOutcome::TapResolved {
                binding_id: active.binding_id,
            },
            HoldResolution::Resolved => {
                // Hold was already resolved (by timeout or interrupt).
                // The release just cleans up state — no new action.
                TapHoldOutcome::PassThrough
            }
        }
    }

    /// Process a repeat event for tap-hold.
    pub(crate) fn on_repeat(&self, key: Key) -> TapHoldOutcome {
        if self.active.contains_key(&key) {
            TapHoldOutcome::RepeatConsumed
        } else {
            TapHoldOutcome::PassThrough
        }
    }

    /// Check for tap-hold timeouts — resolve pending holds past their threshold.
    /// Returns `(key, binding_id)` pairs for newly resolved holds.
    pub(crate) fn check_timeouts(&mut self, now: Instant) -> Vec<(Key, BindingId)> {
        let mut resolved = Vec::new();

        for (key, active) in &mut self.active {
            if active.resolution != HoldResolution::Pending {
                continue;
            }

            let Some(binding) = self.bindings.get(key) else {
                continue;
            };

            let elapsed = now.saturating_duration_since(active.pressed_at);
            if elapsed >= binding.options.threshold() {
                active.resolution = HoldResolution::Resolved;
                resolved.push((*key, active.binding_id));
            }
        }

        resolved
    }

    /// Return the nearest tap-hold timeout deadline, if any pending.
    pub(crate) fn next_deadline(&self, now: Instant) -> Option<Duration> {
        let mut min_remaining: Option<Duration> = None;

        for (key, active) in &self.active {
            if active.resolution != HoldResolution::Pending {
                continue;
            }

            let Some(binding) = self.bindings.get(key) else {
                continue;
            };

            let elapsed = now.saturating_duration_since(active.pressed_at);
            let remaining = binding.options.threshold().saturating_sub(elapsed);
            min_remaining = Some(match min_remaining {
                Some(current) => std::cmp::min(current, remaining),
                None => remaining,
            });
        }

        min_remaining
    }

    /// Get the tap action for a binding by ID.
    pub(crate) fn tap_action(&self, id: BindingId) -> Option<&Action> {
        self.bindings
            .values()
            .find(|b| b.id == id)
            .map(|b| &b.tap_action)
    }

    /// Get the hold action for a binding by ID.
    pub(crate) fn hold_action(&self, id: BindingId) -> Option<&Action> {
        self.bindings
            .values()
            .find(|b| b.id == id)
            .map(|b| &b.hold_action)
    }

    /// Drain interrupt-resolved holds. Returns `(key, binding_id)` pairs
    /// that should be wrapped as `PendingTimeout::TapHoldHold` and handled
    /// through the same pipeline as timeout-resolved holds.
    pub(crate) fn drain_resolved_holds(&mut self) -> Vec<(Key, BindingId)> {
        std::mem::take(&mut self.resolved_holds)
    }

    /// Resolve all pending tap-holds as holds (used by interrupting keypresses).
    /// Excludes the specified key (the one being pressed). Resolved holds are
    /// buffered in `self.resolved_holds` for the engine to drain.
    fn resolve_pending_for_interrupt(&mut self, pressing_key: Key) {
        for (key, active) in &mut self.active {
            if *key == pressing_key {
                continue;
            }
            if active.resolution == HoldResolution::Pending {
                active.resolution = HoldResolution::Resolved;
                self.resolved_holds.push((*key, active.binding_id));
            }
        }
    }
}
