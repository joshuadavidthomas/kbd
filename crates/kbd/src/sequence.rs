//! Sequence-matching configuration and runtime snapshots.

use std::time::Duration;

use crate::error::ParseHotkeyError;
use crate::hotkey::Hotkey;
use crate::hotkey::HotkeySequence;
use crate::key::Key;
use crate::observation::BindingPattern;
use crate::observation::KeyboardObservation;
use crate::observation::LogicalKey;
use crate::observation::LogicalKeyValue;
use crate::observation::NamedKey;
use crate::observation::split_quoted;

/// A non-empty sequence of physical and/or logical binding patterns.
///
/// Unlike [`HotkeySequence`], this is an input matching type, not a physical
/// emission action. Commas outside quoted logical strings separate steps.
///
/// ```
/// use kbd::sequence::BindingSequence;
/// let sequence: BindingSequence = r#"Ctrl+physical:K, logical:",", logical:Enter"#.parse()?;
/// assert_eq!(sequence.steps().len(), 3);
/// # Ok::<(), kbd::error::ParseHotkeyError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BindingSequence {
    steps: Vec<BindingPattern>,
}

impl BindingSequence {
    /// Build a mixed-domain input sequence.
    ///
    /// # Errors
    /// Returns [`ParseHotkeyError::Empty`] for an empty list.
    pub fn new(steps: Vec<BindingPattern>) -> Result<Self, ParseHotkeyError> {
        if steps.is_empty() {
            return Err(ParseHotkeyError::Empty);
        }
        Ok(Self { steps })
    }

    /// Patterns in event order. One press can consume at most one step.
    #[must_use]
    pub fn steps(&self) -> &[BindingPattern] {
        &self.steps
    }

    /// Physical wins at the earliest differing step, after scope/kind priority.
    pub(crate) fn domain_cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.steps
            .iter()
            .map(|step| step.hotkey().is_none())
            .cmp(other.steps.iter().map(|step| step.hotkey().is_none()))
    }
}

impl From<HotkeySequence> for BindingSequence {
    fn from(sequence: HotkeySequence) -> Self {
        Self {
            steps: sequence
                .steps()
                .iter()
                .copied()
                .map(BindingPattern::Physical)
                .collect(),
        }
    }
}

impl std::str::FromStr for BindingSequence {
    type Err = ParseHotkeyError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::new(
            split_quoted(input, ',')?
                .into_iter()
                .map(str::parse)
                .collect::<Result<_, _>>()?,
        )
    }
}

impl std::fmt::Display for BindingSequence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (index, step) in self.steps.iter().enumerate() {
            if index != 0 {
                f.write_str(", ")?;
            }
            write!(f, "{step}")?;
        }
        Ok(())
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for BindingSequence {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for BindingSequence {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let input = String::deserialize(deserializer)?;
        input.parse().map_err(serde::de::Error::custom)
    }
}

/// Identity that cancels a pending sequence, independent of modifiers.
/// An expected next step is matched before the abort identity is considered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SequenceAbortKey {
    /// A physical key (the default is physical Escape).
    Physical(Key),
    /// An exact logical character string or named key.
    Logical(LogicalKey),
}

impl From<Key> for SequenceAbortKey {
    fn from(key: Key) -> Self {
        Self::Physical(key)
    }
}

impl From<LogicalKey> for SequenceAbortKey {
    fn from(key: LogicalKey) -> Self {
        Self::Logical(key)
    }
}

impl From<NamedKey> for SequenceAbortKey {
    fn from(key: NamedKey) -> Self {
        Self::Logical(key.into())
    }
}

impl From<LogicalKeyValue> for SequenceAbortKey {
    fn from(key: LogicalKeyValue) -> Self {
        Self::Logical(key.into())
    }
}

impl SequenceAbortKey {
    pub(crate) fn matches(&self, event: &KeyboardObservation) -> bool {
        match self {
            Self::Physical(key) => event.physical == Some(*key),
            Self::Logical(key) => event.logical.as_ref() == Some(key),
        }
    }
}

mod private {
    pub trait Sealed {}
    impl Sealed for crate::hotkey::HotkeySequence {}
    impl Sealed for Vec<crate::hotkey::Hotkey> {}
    impl Sealed for String {}
    impl Sealed for &str {}
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

impl SequenceInput for Vec<Hotkey> {
    fn into_sequence(self) -> Result<HotkeySequence, ParseHotkeyError> {
        HotkeySequence::new(self)
    }
}

impl SequenceInput for String {
    fn into_sequence(self) -> Result<HotkeySequence, ParseHotkeyError> {
        self.parse()
    }
}

impl SequenceInput for &str {
    fn into_sequence(self) -> Result<HotkeySequence, ParseHotkeyError> {
        self.parse()
    }
}

/// Runtime options for sequence matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceOptions {
    timeout: Duration,
    abort_key: SequenceAbortKey,
}

impl SequenceOptions {
    /// Create sequence options with explicit timeout and abort key.
    #[must_use]
    pub const fn new(timeout: Duration, abort_key: Key) -> Self {
        Self {
            timeout,
            abort_key: SequenceAbortKey::Physical(abort_key),
        }
    }

    /// Timeout for each sequence step.
    #[must_use]
    pub const fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Key that aborts an in-progress sequence.
    #[must_use]
    pub const fn abort_key(&self) -> &SequenceAbortKey {
        &self.abort_key
    }

    /// Set step timeout.
    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set a physical or logical abort identity. Modifiers do not affect matching.
    #[must_use]
    pub fn with_abort_key(mut self, abort_key: impl Into<SequenceAbortKey>) -> Self {
        self.abort_key = abort_key.into();
        self
    }
}

impl Default for SequenceOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_millis(1_000),
            abort_key: SequenceAbortKey::Physical(Key::ESCAPE),
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
