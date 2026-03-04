//! Sequence-matching configuration and runtime snapshots.

use std::time::Duration;

use crate::error::ParseHotkeyError;
use crate::hotkey::HotkeySequence;
use crate::key::Key;

pub(crate) fn parse_sequence(input: &str) -> Result<HotkeySequence, ParseHotkeyError> {
    input.parse()
}

/// Runtime options for sequence matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SequenceOptions {
    timeout: Duration,
    abort_key: Key,
}

impl SequenceOptions {
    /// Create sequence options with explicit timeout and abort key.
    #[must_use]
    pub const fn new(timeout: Duration, abort_key: Key) -> Self {
        Self { timeout, abort_key }
    }

    /// Timeout for each sequence step.
    #[must_use]
    pub const fn timeout(self) -> Duration {
        self.timeout
    }

    /// Key that aborts an in-progress sequence.
    #[must_use]
    pub const fn abort_key(self) -> Key {
        self.abort_key
    }

    /// Set step timeout.
    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set abort key.
    #[must_use]
    pub const fn with_abort_key(mut self, abort_key: Key) -> Self {
        self.abort_key = abort_key;
        self
    }
}

impl Default for SequenceOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_millis(1_000),
            abort_key: Key::ESCAPE,
        }
    }
}

/// Snapshot of current in-progress sequence state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingSequenceInfo {
    /// Number of steps already matched.
    pub steps_matched: usize,
    /// Number of steps still required to complete.
    pub steps_remaining: usize,
}
