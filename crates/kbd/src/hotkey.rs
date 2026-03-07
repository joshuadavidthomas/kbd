//! Hotkey composition types: [`Modifier`], [`Modifiers`], [`Hotkey`], [`HotkeySequence`].
//!
//! These types build on [`Key`] to express key combinations.
//! Parse from human-readable strings (`"Ctrl+Shift+A"`) or build
//! programmatically with [`Hotkey::new`] and [`Hotkey::modifier`].
//!
//! # Parsing
//!
//! ```
//! use kbd::hotkey::{Hotkey, Modifier, Modifiers, HotkeySequence};
//!
//! # fn main() -> Result<(), kbd::error::ParseHotkeyError> {
//! let hotkey: Hotkey = "Ctrl+Shift+A".parse()?;
//! let expected = Modifiers::NONE.with(Modifier::Ctrl).with(Modifier::Shift);
//! assert_eq!(hotkey.modifiers(), expected);
//!
//! let seq: HotkeySequence = "Ctrl+K, Ctrl+C".parse()?;
//! assert_eq!(seq.steps().len(), 2);
//! # Ok(())
//! # }
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

    /// Collect active modifiers from a list of `(flag, modifier)` pairs.
    ///
    /// Bridge crates all perform the same conversion: check each
    /// framework-specific modifier flag and collect the active ones into
    /// a [`Modifiers`]. This helper centralizes that logic.
    ///
    /// # Examples
    ///
    /// ```
    /// use kbd::hotkey::{Modifier, Modifiers};
    ///
    /// let modifiers = Modifier::collect_active([
    ///     (true, Modifier::Ctrl),
    ///     (true, Modifier::Shift),
    ///     (false, Modifier::Alt),
    ///     (false, Modifier::Super),
    /// ]);
    /// let expected = Modifiers::NONE.with(Modifier::Ctrl).with(Modifier::Shift);
    /// assert_eq!(modifiers, expected);
    /// ```
    #[must_use]
    pub fn collect_active<const N: usize>(flags: [(bool, Modifier); N]) -> Modifiers {
        let mut set = Modifiers::NONE;
        for (active, modifier) in flags {
            if active {
                set = set.with(modifier);
            }
        }
        set
    }

    /// Return the bitmask value for this modifier.
    #[must_use]
    const fn bit(self) -> u8 {
        match self {
            Self::Ctrl => 0b0001,
            Self::Shift => 0b0010,
            Self::Alt => 0b0100,
            Self::Super => 0b1000,
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

/// A set of modifier keys represented as a bitmask.
///
/// This is a `Copy` type that stores up to 4 modifiers (Ctrl, Shift, Alt,
/// Super) in a single `u8`. Equality, hashing, and comparison are integer
/// operations — no heap allocation, no pointer chasing.
///
/// # Examples
///
/// ```
/// use kbd::hotkey::{Modifier, Modifiers};
///
/// let mods = Modifiers::NONE
///     .with(Modifier::Ctrl)
///     .with(Modifier::Shift);
///
/// assert!(mods.contains(Modifier::Ctrl));
/// assert!(mods.contains(Modifier::Shift));
/// assert!(!mods.contains(Modifier::Alt));
/// assert_eq!(mods.len(), 2);
///
/// // Iteration yields modifiers in canonical order
/// let collected: Vec<_> = mods.iter().collect();
/// assert_eq!(collected, vec![Modifier::Ctrl, Modifier::Shift]);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct Modifiers(u8);

impl Modifiers {
    /// No modifiers.
    pub const NONE: Self = Self(0);

    /// A modifier set containing only Ctrl.
    pub const CTRL: Self = Self(Modifier::Ctrl.bit());
    /// A modifier set containing only Shift.
    pub const SHIFT: Self = Self(Modifier::Shift.bit());
    /// A modifier set containing only Alt.
    pub const ALT: Self = Self(Modifier::Alt.bit());
    /// A modifier set containing only Super.
    pub const SUPER: Self = Self(Modifier::Super.bit());

    /// All canonical modifiers in order, for iteration.
    const ALL: [Modifier; 4] = [
        Modifier::Ctrl,
        Modifier::Shift,
        Modifier::Alt,
        Modifier::Super,
    ];

    /// Create a modifier set from a raw bitmask.
    ///
    /// Only the lower 4 bits are meaningful. Higher bits are masked off.
    #[must_use]
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits & 0b1111)
    }

    /// Return the raw bitmask.
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Whether this set contains the given modifier.
    #[must_use]
    pub const fn contains(self, modifier: Modifier) -> bool {
        self.0 & modifier.bit() != 0
    }

    /// Return a new set with the given modifier added.
    #[must_use]
    pub const fn with(self, modifier: Modifier) -> Self {
        Self(self.0 | modifier.bit())
    }

    /// Return a new set with the given modifier removed.
    #[must_use]
    pub const fn without(self, modifier: Modifier) -> Self {
        Self(self.0 & !modifier.bit())
    }

    /// Whether the set is empty (no modifiers).
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The number of modifiers in this set.
    #[must_use]
    pub const fn len(self) -> usize {
        self.0.count_ones() as usize
    }

    /// Iterate over the modifiers in this set in canonical order
    /// (Ctrl, Shift, Alt, Super).
    pub fn iter(self) -> impl Iterator<Item = Modifier> {
        let bits = self.0;
        Self::ALL
            .iter()
            .copied()
            .filter(move |m| bits & m.bit() != 0)
    }

    /// Union of two modifier sets.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Intersection of two modifier sets.
    #[must_use]
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}

impl fmt::Debug for Modifiers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

impl fmt::Display for Modifiers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for modifier in self.iter() {
            if !first {
                f.write_str("+")?;
            }
            write!(f, "{modifier}")?;
            first = false;
        }
        Ok(())
    }
}

impl From<Modifier> for Modifiers {
    fn from(modifier: Modifier) -> Self {
        Self(modifier.bit())
    }
}

impl FromIterator<Modifier> for Modifiers {
    fn from_iter<T: IntoIterator<Item = Modifier>>(iter: T) -> Self {
        let mut set = Self::NONE;
        for modifier in iter {
            set = set.with(modifier);
        }
        set
    }
}

impl<'a> FromIterator<&'a Modifier> for Modifiers {
    fn from_iter<T: IntoIterator<Item = &'a Modifier>>(iter: T) -> Self {
        let mut set = Self::NONE;
        for modifier in iter {
            set = set.with(*modifier);
        }
        set
    }
}

impl From<Vec<Modifier>> for Modifiers {
    fn from(modifiers: Vec<Modifier>) -> Self {
        modifiers.iter().collect()
    }
}

impl From<&[Modifier]> for Modifiers {
    fn from(modifiers: &[Modifier]) -> Self {
        modifiers.iter().collect()
    }
}

impl<const N: usize> From<[Modifier; N]> for Modifiers {
    fn from(modifiers: [Modifier; N]) -> Self {
        modifiers.iter().collect()
    }
}

impl IntoIterator for Modifiers {
    type Item = Modifier;
    type IntoIter = ModifiersIter;

    fn into_iter(self) -> Self::IntoIter {
        ModifiersIter {
            bits: self.0,
            index: 0,
        }
    }
}

/// Iterator over the modifiers in a [`Modifiers`].
#[derive(Debug, Clone)]
pub struct ModifiersIter {
    bits: u8,
    index: usize,
}

impl Iterator for ModifiersIter {
    type Item = Modifier;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < Modifiers::ALL.len() {
            let modifier = Modifiers::ALL[self.index];
            self.index += 1;
            if self.bits & modifier.bit() != 0 {
                return Some(modifier);
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.bits >> self.index).count_ones() as usize;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for ModifiersIter {}

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
/// # fn main() -> Result<(), kbd::error::ParseHotkeyError> {
/// // From a string
/// let hotkey: Hotkey = "Ctrl+Shift+A".parse()?;
///
/// // Programmatic
/// let hotkey = Hotkey::new(Key::A).modifier(Modifier::Ctrl).modifier(Modifier::Shift);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hotkey {
    key: Key,
    modifiers: Modifiers,
}

impl Hotkey {
    /// Create a hotkey for a single key with no modifiers.
    #[must_use]
    pub const fn new(key: Key) -> Self {
        Self {
            key,
            modifiers: Modifiers::NONE,
        }
    }

    /// Create a hotkey from a key and a modifier set.
    ///
    /// Accepts anything convertible to a [`Modifiers`] — a `Modifiers`,
    /// a single `Modifier`, a `Vec<Modifier>`, a slice, an array, etc.
    #[must_use]
    pub fn with_modifiers(key: Key, modifiers: impl Into<Modifiers>) -> Self {
        Self {
            key,
            modifiers: modifiers.into(),
        }
    }

    /// Add a modifier to this hotkey.
    #[must_use]
    pub const fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifiers = self.modifiers.with(modifier);
        self
    }

    /// The non-modifier key in this hotkey.
    #[must_use]
    pub const fn key(&self) -> Key {
        self.key
    }

    /// The modifier set required for this hotkey.
    #[must_use]
    pub const fn modifiers(&self) -> Modifiers {
        self.modifiers
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
        let mut modifiers = Modifiers::NONE;
        let mut last_modifier_key = None;
        let mut last_modifier = None;

        for segment in trimmed.split('+') {
            let token = segment.trim();
            if token.is_empty() {
                return Err(ParseHotkeyError::EmptySegment);
            }

            let parsed_key = token
                .parse::<Key>()
                .map_err(|_| ParseHotkeyError::UnknownToken(token.to_string()))?;

            if let Some(modifier) = Modifier::from_key(parsed_key) {
                modifiers = modifiers.with(modifier);
                last_modifier_key = Some(parsed_key);
                last_modifier = Some(modifier);
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
            if let Some(modifier) = last_modifier {
                modifiers = modifiers.without(modifier);
            }
            key
        };

        Ok(Self::with_modifiers(key, modifiers))
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for modifier in self.modifiers {
            write!(f, "{modifier}+")?;
        }

        write!(f, "{}", self.key)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Hotkey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Hotkey {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = <&str>::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
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
/// # fn main() -> Result<(), kbd::error::ParseHotkeyError> {
/// let seq: HotkeySequence = "Ctrl+K, Ctrl+C".parse()?;
/// assert_eq!(seq.steps().len(), 2);
/// # Ok(())
/// # }
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

#[cfg(feature = "serde")]
impl serde::Serialize for HotkeySequence {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for HotkeySequence {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = <&str>::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

mod private {
    pub trait Sealed {}
    impl Sealed for crate::hotkey::Hotkey {}
    impl Sealed for crate::key::Key {}
    impl Sealed for String {}
    impl Sealed for &str {}
}

/// Input types accepted by hotkey registration APIs.
///
/// This trait is intentionally sealed so we can add input forms over time
/// without committing to an open trait-implementation surface.
///
/// Accepts:
/// - [`Hotkey`] — passthrough (infallible)
/// - [`Key`] — wraps in `Hotkey::new(key)` (infallible)
/// - `&str` / `String` — parsed via [`Hotkey::from_str`]
///
/// # Examples
///
/// ```
/// use kbd::hotkey::{Hotkey, HotkeyInput, Modifier};
/// use kbd::key::Key;
///
/// # fn main() -> Result<(), kbd::error::ParseHotkeyError> {
/// // From a Hotkey (infallible)
/// let h = Hotkey::new(Key::A).modifier(Modifier::Ctrl);
/// assert_eq!(h.into_hotkey()?, Hotkey::new(Key::A).modifier(Modifier::Ctrl));
///
/// // From a Key (infallible)
/// assert_eq!(Key::ESCAPE.into_hotkey()?, Hotkey::new(Key::ESCAPE));
///
/// // From a string (parsed)
/// assert_eq!(
///     "Ctrl+A".into_hotkey()?,
///     Hotkey::new(Key::A).modifier(Modifier::Ctrl),
/// );
/// # Ok(())
/// # }
/// ```
pub trait HotkeyInput: private::Sealed {
    /// Converts this input into a [`Hotkey`].
    ///
    /// # Errors
    ///
    /// Returns [`ParseHotkeyError`] when conversion fails (string inputs).
    fn into_hotkey(self) -> Result<Hotkey, ParseHotkeyError>;
}

impl HotkeyInput for Hotkey {
    fn into_hotkey(self) -> Result<Hotkey, ParseHotkeyError> {
        Ok(self)
    }
}

impl HotkeyInput for Key {
    fn into_hotkey(self) -> Result<Hotkey, ParseHotkeyError> {
        Ok(Hotkey::from(self))
    }
}

impl HotkeyInput for String {
    fn into_hotkey(self) -> Result<Hotkey, ParseHotkeyError> {
        self.parse()
    }
}

impl HotkeyInput for &str {
    fn into_hotkey(self) -> Result<Hotkey, ParseHotkeyError> {
        self.parse()
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
        let expected = Modifiers::NONE.with(Modifier::Ctrl).with(Modifier::Super);
        assert_eq!(hotkey.modifiers(), expected);
    }

    #[test]
    fn hotkey_new_dedups_modifiers() {
        let hotkey =
            Hotkey::with_modifiers(Key::A, vec![Modifier::Alt, Modifier::Ctrl, Modifier::Alt]);
        let expected = Modifiers::NONE.with(Modifier::Ctrl).with(Modifier::Alt);
        assert_eq!(hotkey.modifiers(), expected);
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
        assert_eq!(hotkey.modifiers(), Modifiers::CTRL);
    }

    #[test]
    fn collect_active_returns_only_active_modifiers() {
        let modifiers = Modifier::collect_active([
            (true, Modifier::Ctrl),
            (false, Modifier::Shift),
            (true, Modifier::Alt),
            (false, Modifier::Super),
        ]);
        let expected = Modifiers::NONE.with(Modifier::Ctrl).with(Modifier::Alt);
        assert_eq!(modifiers, expected);
    }

    #[test]
    fn collect_active_all_true() {
        let modifiers = Modifier::collect_active([
            (true, Modifier::Ctrl),
            (true, Modifier::Shift),
            (true, Modifier::Alt),
            (true, Modifier::Super),
        ]);
        let expected = Modifiers::NONE
            .with(Modifier::Ctrl)
            .with(Modifier::Shift)
            .with(Modifier::Alt)
            .with(Modifier::Super);
        assert_eq!(modifiers, expected);
    }

    #[test]
    fn collect_active_none_true() {
        let modifiers = Modifier::collect_active([
            (false, Modifier::Ctrl),
            (false, Modifier::Shift),
            (false, Modifier::Alt),
            (false, Modifier::Super),
        ]);
        assert!(modifiers.is_empty());
    }

    #[test]
    fn collect_active_single() {
        let modifiers = Modifier::collect_active([(true, Modifier::Shift)]);
        assert_eq!(modifiers, Modifiers::SHIFT);
    }

    #[test]
    fn collect_active_empty_array() {
        let modifiers = Modifier::collect_active([]);
        assert!(modifiers.is_empty());
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

    #[test]
    fn hotkey_input_from_hotkey_is_infallible() {
        let hotkey = Hotkey::new(Key::A).modifier(Modifier::Ctrl);
        let result = hotkey.into_hotkey();
        assert_eq!(
            result.unwrap(),
            Hotkey::new(Key::A).modifier(Modifier::Ctrl)
        );
    }

    #[test]
    fn hotkey_input_from_key_wraps_in_hotkey() {
        let result = Key::A.into_hotkey();
        assert_eq!(result.unwrap(), Hotkey::new(Key::A));
    }

    #[test]
    fn hotkey_input_from_str_parses() {
        let result = "Ctrl+A".into_hotkey();
        assert_eq!(
            result.unwrap(),
            Hotkey::new(Key::A).modifier(Modifier::Ctrl)
        );
    }

    #[test]
    fn hotkey_input_from_string_parses() {
        let result = String::from("Ctrl+A").into_hotkey();
        assert_eq!(
            result.unwrap(),
            Hotkey::new(Key::A).modifier(Modifier::Ctrl)
        );
    }

    #[test]
    fn hotkey_input_from_str_reports_parse_error() {
        let result = "Ctrl+Nope".into_hotkey();
        assert!(matches!(result, Err(ParseHotkeyError::UnknownToken(_))));
    }

    #[test]
    fn hotkey_input_from_string_reports_parse_error() {
        let result = String::from("Ctrl+Nope").into_hotkey();
        assert!(matches!(result, Err(ParseHotkeyError::UnknownToken(_))));
    }
}
