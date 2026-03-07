//! Held-key state — tracks engine decisions for keys that are currently pressed.
//!
//! When a key is pressed, the engine decides how to handle it (forward, consume,
//! etc.) and may capture repeat state (callback, timing). This module stores
//! those decisions for the duration of the press-release cycle so that
//! subsequent release and repeat events behave consistently — even if the
//! active layer changes mid-press (oneshot, `PopLayer` action, etc.).
//!
//! Mirrors the approach used by keyd's `cache_entry` system.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use kbd::action::Action;
use kbd::hotkey::Modifier;
use kbd::key::Key;
use kbd::policy::RepeatPolicy;

use super::types::KeyEventOutcome;

/// Tracks engine decisions for keys that are currently held down.
///
/// Each non-modifier key that is pressed gets an entry recording its
/// [`KeyEventOutcome`] and optional [`RepeatState`]. The entry is removed
/// on release, ensuring each press-release cycle has exactly one entry
/// lifetime.
///
/// Modifier keys are excluded — they don't go through binding matching.
pub(super) struct HeldKeyState(HashMap<Key, HeldKeyStateEntry>);

impl HeldKeyState {
    pub(super) fn new() -> Self {
        Self(HashMap::new())
    }

    /// Record a press outcome in the cache so release and repeat events
    /// use the same disposition. Modifier keys are excluded — they don't
    /// go through binding matching.
    pub(super) fn insert(
        &mut self,
        key: Key,
        outcome: KeyEventOutcome,
        repeat_state: Option<RepeatState>,
    ) {
        if Modifier::from_key(key).is_none() {
            self.0.insert(
                key,
                HeldKeyStateEntry {
                    outcome,
                    repeat_state,
                },
            );
        }
    }

    /// Remove and return the cached entry for a key (on release).
    pub(super) fn remove(&mut self, key: Key) -> Option<HeldKeyStateEntry> {
        self.0.remove(&key)
    }

    /// Get the cached entry for a key (for repeat event handling).
    pub(super) fn get(&self, key: Key) -> Option<&HeldKeyStateEntry> {
        self.0.get(&key)
    }

    /// Get a mutable reference to the cached entry (for updating repeat timing).
    pub(super) fn get_mut(&mut self, key: Key) -> Option<&mut HeldKeyStateEntry> {
        self.0.get_mut(&key)
    }
}

/// Everything the engine decided about a key that is currently held down.
///
/// Stores the press disposition (for release forwarding) and optional
/// repeat state (for callback re-firing and timing). Ensures release and
/// repeat events behave consistently with the original press — even
/// across layer transitions.
pub(super) struct HeldKeyStateEntry {
    /// The original forwarding disposition from the press event.
    pub(super) outcome: KeyEventOutcome,
    /// Repeat handling state for matched bindings.
    pub(super) repeat_state: Option<RepeatState>,
}

/// Repeat handling state for a matched binding.
///
/// Caches the callback from the original press so repeat events can
/// re-fire it without re-querying the dispatcher (which would trigger
/// debounce, rate limiting, and layer side effects).
pub(super) struct RepeatState {
    /// The callback to re-fire on repeat, if the action was a callback.
    ///
    /// Layer actions, suppress, and emit don't repeat — only user
    /// callbacks do.
    pub(super) callback: Option<Arc<dyn Fn() + Send + Sync>>,
    /// How repeat events should be handled.
    pub(super) policy: RepeatPolicy,
    /// When the original press occurred (for Custom delay tracking).
    pub(super) press_time: Instant,
    /// When the last repeat action fired (for Custom rate tracking).
    pub(super) last_repeat_fire: Option<Instant>,
}

impl RepeatState {
    /// Build repeat state from a matched action and its repeat policy.
    ///
    /// Captures the callback (if any) and records the current time as
    /// the press time for Custom delay/rate tracking.
    pub(super) fn for_action(action: &Action, policy: RepeatPolicy) -> Self {
        let callback = match action {
            Action::Callback(cb) => Some(Arc::clone(cb)),
            _ => None,
        };
        Self {
            callback,
            policy,
            press_time: Instant::now(),
            last_repeat_fire: None,
        }
    }
}
