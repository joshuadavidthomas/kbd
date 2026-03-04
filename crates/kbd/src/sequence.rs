//! Sequence-matching configuration and runtime snapshots.

use std::time::Duration;

use crate::error::ParseHotkeyError;
use crate::hotkey::Hotkey;
use crate::hotkey::HotkeySequence;
use crate::key::Key;

mod private {
    pub trait Sealed {}
    impl Sealed for crate::hotkey::HotkeySequence {}
    impl Sealed for &str {}
    impl Sealed for String {}
    impl Sealed for Vec<crate::hotkey::Hotkey> {}
}

/// Input types accepted by sequence registration APIs.
///
/// This trait is intentionally sealed so we can add input forms over time
/// without committing to an open trait-implementation surface.
pub trait SequenceInput: private::Sealed {
    /// Converts this input into a [`HotkeySequence`].
    ///
    /// # Errors
    ///
    /// Returns [`ParseHotkeyError`] when conversion fails.
    fn into_sequence(self) -> Result<HotkeySequence, ParseHotkeyError>;
}

impl SequenceInput for HotkeySequence {
    fn into_sequence(self) -> Result<HotkeySequence, ParseHotkeyError> {
        Ok(self)
    }
}

impl SequenceInput for &str {
    fn into_sequence(self) -> Result<HotkeySequence, ParseHotkeyError> {
        self.parse()
    }
}

impl SequenceInput for String {
    fn into_sequence(self) -> Result<HotkeySequence, ParseHotkeyError> {
        self.parse()
    }
}

impl SequenceInput for Vec<Hotkey> {
    fn into_sequence(self) -> Result<HotkeySequence, ParseHotkeyError> {
        HotkeySequence::new(self)
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hotkey::Modifier;

    #[test]
    fn typed_sequence_input_round_trips() {
        let sequence = HotkeySequence::new(vec![Hotkey::new(Key::K), Hotkey::new(Key::C)])
            .expect("valid sequence");
        let parsed = <HotkeySequence as SequenceInput>::into_sequence(sequence.clone())
            .expect("typed sequence input should not fail");
        assert_eq!(parsed, sequence);
    }

    #[test]
    fn str_sequence_input_parses() {
        let parsed = <&str as SequenceInput>::into_sequence("Ctrl+K, Ctrl+C")
            .expect("valid sequence string should parse");
        let expected = HotkeySequence::new(vec![
            Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            Hotkey::new(Key::C).modifier(Modifier::Ctrl),
        ])
        .expect("valid expected sequence");
        assert_eq!(parsed, expected);
    }

    #[test]
    fn str_sequence_input_reports_parse_error() {
        let parsed = <&str as SequenceInput>::into_sequence("Ctrl+K, Ctrl+Nope");
        assert!(matches!(parsed, Err(ParseHotkeyError::UnknownToken(_))));
    }

    #[test]
    fn string_sequence_input_parses() {
        let parsed = <String as SequenceInput>::into_sequence("Ctrl+K, Ctrl+C".to_string())
            .expect("valid sequence string should parse");
        let expected = HotkeySequence::new(vec![
            Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            Hotkey::new(Key::C).modifier(Modifier::Ctrl),
        ])
        .expect("valid expected sequence");
        assert_eq!(parsed, expected);
    }

    #[test]
    fn vec_hotkey_sequence_input_builds_sequence() {
        let parsed = <Vec<Hotkey> as SequenceInput>::into_sequence(vec![
            Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            Hotkey::new(Key::C).modifier(Modifier::Ctrl),
        ])
        .expect("valid vec hotkey input should build a sequence");

        let expected = HotkeySequence::new(vec![
            Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            Hotkey::new(Key::C).modifier(Modifier::Ctrl),
        ])
        .expect("valid expected sequence");
        assert_eq!(parsed, expected);
    }

    #[test]
    fn vec_hotkey_sequence_input_rejects_empty_sequence() {
        let parsed = <Vec<Hotkey> as SequenceInput>::into_sequence(Vec::new());
        assert!(matches!(parsed, Err(ParseHotkeyError::Empty)));
    }
}
