//! Hotkey composition types: [`Modifier`], [`Hotkey`], [`HotkeySequence`].
//!
//! These types build on [`Key`](crate::key::Key) to express key combinations.
//! Parse from human-readable strings (`"Ctrl+Shift+A"`) or build
//! programmatically with [`Hotkey::new`] and [`Hotkey::modifier`].
//!
//! # Parsing
//!
//! ```
//! use kbd::hotkey::{Hotkey, Modifier, HotkeySequence};
//!
//! let hotkey: Hotkey = "Ctrl+Shift+A".parse().unwrap();
//! assert_eq!(hotkey.modifiers(), &[Modifier::Ctrl, Modifier::Shift]);
//!
//! let seq: HotkeySequence = "Ctrl+K, Ctrl+C".parse().unwrap();
//! assert_eq!(seq.steps().len(), 2);
//! ```

use std::fmt;
use std::str::FromStr;

use keyboard_types::Code;

use crate::error::ParseHotkeyError;
use crate::key::Key;

/// A canonical modifier key (Ctrl, Shift, Alt, Super).
///
/// Left and right physical variants are canonicalized — both `ControlLeft`
/// and `ControlRight` map to `Modifier::Ctrl`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Modifier {
    /// The Control modifier (left or right).
    Ctrl,
    /// The Shift modifier (left or right).
    Shift,
    /// The Alt modifier (left or right).
    Alt,
    /// The Super/Meta/Win modifier (left or right).
    Super,
}

impl Modifier {
    /// Human-readable name for this modifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ctrl => "Ctrl",
            Self::Shift => "Shift",
            Self::Alt => "Alt",
            Self::Super => "Super",
        }
    }

    /// Check whether a key is a modifier key, returning the canonical modifier.
    ///
    /// Left/right variants canonicalize: both `ControlLeft` and `ControlRight`
    /// return `Some(Modifier::Ctrl)`.
    #[must_use]
    pub fn from_key(key: Key) -> Option<Self> {
        match Code::from(key) {
            Code::ControlLeft | Code::ControlRight => Some(Self::Ctrl),
            Code::ShiftLeft | Code::ShiftRight => Some(Self::Shift),
            Code::AltLeft | Code::AltRight => Some(Self::Alt),
            Code::MetaLeft | Code::MetaRight => Some(Self::Super),
            _ => None,
        }
    }

    /// Return the left and right physical [`Key`] variants for this modifier.
    #[must_use]
    pub const fn keys(self) -> (Key, Key) {
        match self {
            Self::Ctrl => (Key::CONTROL_LEFT, Key::CONTROL_RIGHT),
            Self::Shift => (Key::SHIFT_LEFT, Key::SHIFT_RIGHT),
            Self::Alt => (Key::ALT_LEFT, Key::ALT_RIGHT),
            Self::Super => (Key::META_LEFT, Key::META_RIGHT),
        }
    }
}

impl fmt::Display for Modifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Modifier {
    type Err = ParseHotkeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let token = s.trim();
        let lower = token.to_ascii_lowercase();

        if let Some(modifier) = match lower.as_str() {
            "ctrl" | "control" => Some(Self::Ctrl),
            "shift" => Some(Self::Shift),
            "alt" => Some(Self::Alt),
            "super" | "meta" | "win" | "windows" => Some(Self::Super),
            _ => None,
        } {
            return Ok(modifier);
        }

        token
            .parse::<Key>()
            .ok()
            .and_then(Self::from_key)
            .ok_or_else(|| ParseHotkeyError::UnknownToken(token.to_string()))
    }
}

impl TryFrom<Key> for Modifier {
    type Error = Key;

    fn try_from(value: Key) -> Result<Self, Self::Error> {
        Self::from_key(value).ok_or(value)
    }
}

impl From<Modifier> for Key {
    fn from(value: Modifier) -> Self {
        value.keys().0
    }
}

/// A key combined with zero or more modifiers.
///
/// Hotkeys are the matching unit — `"Ctrl+C"`, `"Shift+F5"`, or just `"Escape"`.
/// Parse from strings with [`str::parse`] or build programmatically with
/// [`Hotkey::new`] and [`Hotkey::modifier`].
///
/// ```
/// use kbd::hotkey::{Hotkey, Modifier};
/// use kbd::key::Key;
///
/// // From a string
/// let hotkey: Hotkey = "Ctrl+Shift+A".parse().unwrap();
///
/// // Programmatic
/// let hotkey = Hotkey::new(Key::A).modifier(Modifier::Ctrl).modifier(Modifier::Shift);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Hotkey {
    key: Key,
    modifiers: Vec<Modifier>,
}

impl Hotkey {
    /// Create a hotkey for a single key with no modifiers.
    #[must_use]
    pub fn new(key: Key) -> Self {
        Self {
            key,
            modifiers: Vec::new(),
        }
    }

    /// Create a hotkey from a key and a list of modifiers. Modifiers are
    /// sorted and deduplicated.
    ///
    /// Accepts anything convertible to a `Vec<Modifier>` — a `Vec`, a slice,
    /// an array, etc.
    #[must_use]
    pub fn with_modifiers(key: Key, modifiers: impl Into<Vec<Modifier>>) -> Self {
        let mut modifiers = modifiers.into();
        modifiers.sort();
        modifiers.dedup();
        Self { key, modifiers }
    }

    /// Add a modifier to this hotkey.
    #[must_use]
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        if !self.modifiers.contains(&modifier) {
            self.modifiers.push(modifier);
            self.modifiers.sort();
        }
        self
    }

    /// The non-modifier key in this hotkey.
    #[must_use]
    pub fn key(&self) -> Key {
        self.key
    }

    /// The modifiers required for this hotkey (sorted, deduplicated).
    #[must_use]
    pub fn modifiers(&self) -> &[Modifier] {
        &self.modifiers
    }
}

impl From<Key> for Hotkey {
    fn from(key: Key) -> Self {
        Self::new(key)
    }
}

impl FromStr for Hotkey {
    type Err = ParseHotkeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(ParseHotkeyError::Empty);
        }

        let mut key = None;
        let mut modifiers = Vec::new();
        let mut last_modifier_key = None;

        for segment in trimmed.split('+') {
            let token = segment.trim();
            if token.is_empty() {
                return Err(ParseHotkeyError::EmptySegment);
            }

            let parsed_key = token
                .parse::<Key>()
                .map_err(|_| ParseHotkeyError::UnknownToken(token.to_string()))?;

            if let Some(modifier) = Modifier::from_key(parsed_key) {
                modifiers.push(modifier);
                last_modifier_key = Some(parsed_key);
                continue;
            }

            if key.replace(parsed_key).is_some() {
                return Err(ParseHotkeyError::MultipleKeys);
            }
        }

        let key = if let Some(key) = key {
            key
        } else {
            let key = last_modifier_key.ok_or(ParseHotkeyError::MissingKey)?;
            modifiers.pop();
            key
        };

        Ok(Self::with_modifiers(key, modifiers))
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for modifier in &self.modifiers {
            write!(f, "{modifier}+")?;
        }

        write!(f, "{}", self.key)
    }
}

/// A multi-step hotkey sequence like `"Ctrl+K, Ctrl+C"`.
///
/// Sequences are comma-separated hotkeys. Each step must be pressed
/// in order for the sequence to match.
///
/// ```
/// use kbd::hotkey::HotkeySequence;
///
/// let seq: HotkeySequence = "Ctrl+K, Ctrl+C".parse().unwrap();
/// assert_eq!(seq.steps().len(), 2);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HotkeySequence {
    steps: Vec<Hotkey>,
}

impl HotkeySequence {
    /// Create a sequence from a non-empty list of hotkeys.
    ///
    /// # Errors
    ///
    /// Returns [`ParseHotkeyError`] if `steps` is empty.
    pub fn new(steps: Vec<Hotkey>) -> Result<Self, ParseHotkeyError> {
        if steps.is_empty() {
            return Err(ParseHotkeyError::Empty);
        }

        Ok(Self { steps })
    }

    /// The individual hotkey steps in this sequence.
    #[must_use]
    pub fn steps(&self) -> &[Hotkey] {
        &self.steps
    }
}

impl FromStr for HotkeySequence {
    type Err = ParseHotkeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut steps = Vec::new();
        for segment in s.split(',') {
            steps.push(segment.trim().parse::<Hotkey>()?);
        }

        Self::new(steps)
    }
}

impl fmt::Display for HotkeySequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, step) in self.steps.iter().enumerate() {
            if index > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{step}")?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::Key;

    #[test]
    fn parses_aliases_case_insensitive() {
        let hotkey = "ctrl+Win+return".parse::<Hotkey>().unwrap();
        assert_eq!(hotkey.key(), Key::ENTER);
        assert_eq!(hotkey.modifiers(), &[Modifier::Ctrl, Modifier::Super]);
    }

    #[test]
    fn hotkey_new_sorts_and_dedups_modifiers() {
        let hotkey =
            Hotkey::with_modifiers(Key::A, vec![Modifier::Alt, Modifier::Ctrl, Modifier::Alt]);
        assert_eq!(hotkey.modifiers(), &[Modifier::Ctrl, Modifier::Alt]);
    }

    #[test]
    fn modifier_canonicalizes_left_and_right_keys() {
        assert_eq!(Modifier::from_key(Key::CONTROL_LEFT), Some(Modifier::Ctrl));
        assert_eq!(Modifier::from_key(Key::CONTROL_RIGHT), Some(Modifier::Ctrl));
        assert_eq!(Modifier::from_key(Key::SHIFT_LEFT), Some(Modifier::Shift));
        assert_eq!(Modifier::from_key(Key::SHIFT_RIGHT), Some(Modifier::Shift));
        assert_eq!(Modifier::from_key(Key::ALT_LEFT), Some(Modifier::Alt));
        assert_eq!(Modifier::from_key(Key::ALT_RIGHT), Some(Modifier::Alt));
        assert_eq!(Modifier::from_key(Key::META_LEFT), Some(Modifier::Super));
        assert_eq!(Modifier::from_key(Key::META_RIGHT), Some(Modifier::Super));
        assert_eq!(Modifier::from_key(Key::A), None);
    }

    #[test]
    fn parses_modifier_key_as_trigger_when_no_non_modifier_key_exists() {
        let hotkey = "Ctrl".parse::<Hotkey>().unwrap();
        assert_eq!(hotkey.key(), Key::CONTROL_LEFT);
        assert!(hotkey.modifiers().is_empty());
    }

    #[test]
    fn parses_all_modifier_combo_with_last_modifier_as_trigger() {
        let hotkey = "Ctrl+Shift".parse::<Hotkey>().unwrap();
        assert_eq!(hotkey.key(), Key::SHIFT_LEFT);
        assert_eq!(hotkey.modifiers(), &[Modifier::Ctrl]);
    }

    #[test]
    fn parses_extended_key_ranges() {
        let cases = [
            ("F24", Key::F24),
            ("Left", Key::ARROW_LEFT),
            ("Delete", Key::DELETE),
            ("Backspace", Key::BACKSPACE),
            ("Insert", Key::INSERT),
            ("Home", Key::HOME),
            ("End", Key::END),
            ("PageUp", Key::PAGE_UP),
            ("PageDown", Key::PAGE_DOWN),
            ("Numpad1", Key::NUMPAD1),
            ("NumpadEnter", Key::NUMPAD_ENTER),
            ("Equal", Key::EQUAL),
            ("Minus", Key::MINUS),
            ("Comma", Key::COMMA),
            ("Slash", Key::SLASH),
        ];

        for (input, expected) in cases {
            let hotkey = format!("Ctrl+{input}").parse::<Hotkey>().unwrap();
            assert_eq!(hotkey.key(), expected, "failed parsing {input}");

            let round_trip = hotkey.to_string().parse::<Hotkey>().unwrap();
            assert_eq!(round_trip, hotkey, "failed round-trip for {input}");
        }
    }
}
