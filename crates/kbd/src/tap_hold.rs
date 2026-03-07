//! Tap-hold binding types — dual-function keys with time-based resolution.
//!
//! A tap-hold binding assigns two actions to a single key: one for a quick
//! tap (press + release within a threshold), another for a sustained hold
//! (held past the threshold or interrupted by another keypress).
//!
//! # Resolution rules
//!
//! 1. **Tap**: key released before the threshold → tap action fires
//! 2. **Hold by duration**: key held past the threshold → hold action fires
//! 3. **Hold by interrupt**: another key pressed while pending → hold action
//!    fires immediately (keyd model)
//!
//! # Grab mode requirement
//!
//! Tap-hold requires grab mode in `kbd-global` because it must intercept and
//! buffer key events before they reach other applications. Without grab, the
//! original key event would be delivered immediately, making tap-vs-hold
//! disambiguation impossible.

use std::time::Duration;

/// Options for tap-hold behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TapHoldOptions {
    threshold: Duration,
}

impl Default for TapHoldOptions {
    fn default() -> Self {
        Self {
            threshold: Duration::from_millis(200),
        }
    }
}

impl TapHoldOptions {
    /// Create default options (200ms threshold).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the duration threshold: released before = tap, held past = hold.
    #[must_use]
    pub fn with_threshold(mut self, threshold: Duration) -> Self {
        self.threshold = threshold;
        self
    }

    /// Get the configured threshold duration.
    #[must_use]
    pub fn threshold(&self) -> Duration {
        self.threshold
    }
}
