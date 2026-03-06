//! Hotkey composition types: [`Modifier`], [`Hotkey`], [`HotkeySequence`].
//!
//! These types build on [`Key`] to express key combinations.
//! Parse from human-readable strings (`"Ctrl+Shift+A"`) or build
//! programmatically with [`Hotkey::new`] and [`Hotkey::modifier`].
//!
//! # Parsing
//!
//! ```
//! use kbd::hotkey::{Hotkey, Modifier, HotkeySequence};
//!
//! # fn main() -> Result<(), kbd::error::ParseHotkeyError> {
//! let hotkey: Hotkey = "Ctrl+Shift+A".parse()?;
//! assert_eq!(hotkey.modifiers(), &[Modifier::Ctrl, Modifier::Shift]);
//!
//! let seq: HotkeySequence = "Ctrl+K, Ctrl+C".parse()?;
//! assert_eq!(seq.steps().len(), 2);
//! # Ok(())
//! # }
//! ```

use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::str::FromStr;

use keyboard_types::Code;

use crate::error::ParseHotkeyError;
use crate::key::Key;

/// A user-defined modifier alias name.
///
/// Aliases let users define abstract modifier names like `"Mod"` that
/// resolve to concrete modifiers (`Ctrl`, `Shift`, `Alt`, `Super`) at
/// match time. This enables portable bindings across different alias
/// configurations — for example, a tiling WM can define `"Mod"` as
/// `Super` and let users rebind it to `Alt` without changing any hotkey
/// definitions.
///
/// Alias names are case-preserving. Resolution through the
/// [`Dispatcher`](crate::dispatcher::Dispatcher) is case-insensitive
/// (defining `"Mod"` and looking up `"mod"` reaches the same alias),
/// and equality/hash/order follow that same case-insensitive behavior.
#[derive(Debug, Clone, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ModifierAlias(String);

impl ModifierAlias {
    /// Create a new modifier alias from a name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// The alias name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl PartialEq for ModifierAlias {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq_ignore_ascii_case(&other.0)
    }
}

impl Hash for ModifierAlias {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for byte in self.0.bytes() {
            state.write_u8(byte.to_ascii_lowercase());
        }
    }
}

impl PartialOrd for ModifierAlias {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ModifierAlias {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0
            .bytes()
            .map(|byte| byte.to_ascii_lowercase())
            .cmp(other.0.bytes().map(|byte| byte.to_ascii_lowercase()))
    }
}

impl fmt::Display for ModifierAlias {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Maps alias names to the concrete modifiers they resolve to.
pub type ModifierAliases = HashMap<ModifierAlias, Modifier>;

/// A canonical modifier key (Ctrl, Shift, Alt, Super) or a user-defined alias.
///
/// Left and right physical variants are canonicalized — both `ControlLeft`
/// and `ControlRight` map to `Modifier::Ctrl`.
///
/// The `Alias` variant stores a user-defined modifier name that is
/// resolved at match time by the [`Dispatcher`](crate::dispatcher::Dispatcher).
/// This allows bindings to be portable across alias configurations.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
    /// A user-defined modifier alias, resolved at match time.
    Alias(ModifierAlias),
}

impl Modifier {
    /// Human-readable name for this modifier.
    ///
    /// Returns the canonical name for concrete modifiers and the alias
    /// name for [`Modifier::Alias`] variants.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Ctrl => "Ctrl",
            Self::Shift => "Shift",
            Self::Alt => "Alt",
            Self::Super => "Super",
            Self::Alias(alias) => alias.as_str(),
        }
    }

    /// Check whether a key is a modifier key, returning the canonical modifier.
    ///
    /// Left/right variants canonicalize: both `ControlLeft` and `ControlRight`
    /// return `Some(Modifier::Ctrl)`.
    ///
    /// Never returns `Modifier::Alias` — aliases are a parsing/configuration
    /// concept, not a physical key property.
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
    ///
    /// Returns `None` for [`Modifier::Alias`] — aliases don't correspond
    /// to physical keys until resolved.
    #[must_use]
    pub fn keys(&self) -> Option<(Key, Key)> {
        match self {
            Self::Ctrl => Some((Key::CONTROL_LEFT, Key::CONTROL_RIGHT)),
            Self::Shift => Some((Key::SHIFT_LEFT, Key::SHIFT_RIGHT)),
            Self::Alt => Some((Key::ALT_LEFT, Key::ALT_RIGHT)),
            Self::Super => Some((Key::META_LEFT, Key::META_RIGHT)),
            Self::Alias(_) => None,
        }
    }

    /// Collect active modifiers from a list of `(flag, modifier)` pairs.
    ///
    /// Bridge crates all perform the same conversion: check each
    /// framework-specific modifier flag and collect the active ones into
    /// a `Vec<Modifier>`. This helper centralizes that logic.
    ///
    /// # Examples
    ///
    /// ```
    /// use kbd::hotkey::Modifier;
    ///
    /// let modifiers = Modifier::collect_active([
    ///     (true, Modifier::Ctrl),
    ///     (true, Modifier::Shift),
    ///     (false, Modifier::Alt),
    ///     (false, Modifier::Super),
    /// ]);
    /// assert_eq!(modifiers, vec![Modifier::Ctrl, Modifier::Shift]);
    /// ```
    #[must_use]
    pub fn collect_active<const N: usize>(flags: [(bool, Modifier); N]) -> Vec<Modifier> {
        let mut modifiers = Vec::with_capacity(N);
        for (active, modifier) in flags {
            if active {
                modifiers.push(modifier);
            }
        }
        modifiers
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

        // Try parsing as a physical key that happens to be a modifier
        if let Ok(key) = token.parse::<Key>() {
            if let Some(modifier) = Self::from_key(key) {
                return Ok(modifier);
            }
            // Valid key but not a modifier — not an alias either
            return Err(ParseHotkeyError::UnknownToken(token.to_string()));
        }

        // Not a known key — treat alphabetic tokens as user-defined aliases
        if token
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic())
        {
            return Ok(Self::Alias(ModifierAlias::new(token)));
        }

        Err(ParseHotkeyError::UnknownToken(token.to_string()))
    }
}

impl TryFrom<Key> for Modifier {
    type Error = Key;

    fn try_from(value: Key) -> Result<Self, Self::Error> {
        Self::from_key(value).ok_or(value)
    }
}

impl TryFrom<Modifier> for Key {
    type Error = ModifierAlias;

    /// Convert a concrete modifier to its left-side physical key.
    ///
    /// Returns `Err(alias)` for [`Modifier::Alias`] — aliases don't
    /// correspond to physical keys until resolved.
    fn try_from(value: Modifier) -> Result<Self, Self::Error> {
        match value {
            Modifier::Ctrl => Ok(Key::CONTROL_LEFT),
            Modifier::Shift => Ok(Key::SHIFT_LEFT),
            Modifier::Alt => Ok(Key::ALT_LEFT),
            Modifier::Super => Ok(Key::META_LEFT),
            Modifier::Alias(alias) => Err(alias),
        }
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
/// # fn main() -> Result<(), kbd::error::ParseHotkeyError> {
/// // From a string
/// let hotkey: Hotkey = "Ctrl+Shift+A".parse()?;
///
/// // Programmatic
/// let hotkey = Hotkey::new(Key::A).modifier(Modifier::Ctrl).modifier(Modifier::Shift);
/// # Ok(())
/// # }
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

            // Try parsing as a known key first
            if let Ok(parsed_key) = token.parse::<Key>() {
                if let Some(modifier) = Modifier::from_key(parsed_key) {
                    modifiers.push(modifier);
                    last_modifier_key = Some(parsed_key);
                    continue;
                }

                if key.replace(parsed_key).is_some() {
                    return Err(ParseHotkeyError::MultipleKeys);
                }
                continue;
            }

            // Not a recognized key — delegate to Modifier parsing, which
            // handles concrete modifier names, user-defined aliases, and
            // error reporting in one place.
            modifiers.push(token.parse::<Modifier>()?);
        }

        let key = if let Some(key) = key {
            key
        } else {
            let key = last_modifier_key.ok_or(ParseHotkeyError::MissingKey)?;
            // Remove the modifier that corresponds to the trigger key.
            // We can't blindly pop() because alias modifiers may have been
            // pushed after the last physical modifier.
            let trigger_modifier =
                Modifier::from_key(key).expect("last_modifier_key is always a modifier key");
            if let Some(pos) = modifiers.iter().rposition(|m| m == &trigger_modifier) {
                modifiers.remove(pos);
            }
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
        assert_eq!(hotkey.modifiers(), &[Modifier::Ctrl, Modifier::Super]);
    }

    #[test]
    fn modifier_from_str_parses_concrete_names() {
        assert_eq!("Ctrl".parse::<Modifier>().unwrap(), Modifier::Ctrl);
        assert_eq!("shift".parse::<Modifier>().unwrap(), Modifier::Shift);
        assert_eq!("Alt".parse::<Modifier>().unwrap(), Modifier::Alt);
        assert_eq!("Super".parse::<Modifier>().unwrap(), Modifier::Super);
        assert_eq!("Meta".parse::<Modifier>().unwrap(), Modifier::Super);
        assert_eq!("Win".parse::<Modifier>().unwrap(), Modifier::Super);
    }

    #[test]
    fn modifier_from_str_parses_alias() {
        let modifier = "Mod".parse::<Modifier>().unwrap();
        assert_eq!(modifier, Modifier::Alias(ModifierAlias::new("Mod")));
    }

    #[test]
    fn modifier_alias_equality_is_case_insensitive() {
        assert_eq!(ModifierAlias::new("Mod"), ModifierAlias::new("mod"));
    }

    #[test]
    fn hotkey_dedups_alias_modifiers_case_insensitively() {
        let hotkey = Hotkey::with_modifiers(
            Key::T,
            vec![
                Modifier::Alias(ModifierAlias::new("Mod")),
                Modifier::Alias(ModifierAlias::new("mod")),
            ],
        );
        assert_eq!(hotkey.modifiers().len(), 1);
        assert!(matches!(
            &hotkey.modifiers()[0],
            Modifier::Alias(alias) if alias.as_str().eq_ignore_ascii_case("mod")
        ));
    }

    #[test]
    fn modifier_from_str_rejects_non_modifier_key_name() {
        // "Space" is a valid key but not a modifier — should not become an alias
        let result = "Space".parse::<Modifier>();
        assert!(matches!(result, Err(ParseHotkeyError::UnknownToken(_))));
    }

    #[test]
    fn modifier_from_str_rejects_non_alphabetic_token() {
        let result = "@@@".parse::<Modifier>();
        assert!(matches!(result, Err(ParseHotkeyError::UnknownToken(_))));
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
    fn collect_active_returns_only_active_modifiers() {
        let modifiers = Modifier::collect_active([
            (true, Modifier::Ctrl),
            (false, Modifier::Shift),
            (true, Modifier::Alt),
            (false, Modifier::Super),
        ]);
        assert_eq!(modifiers, vec![Modifier::Ctrl, Modifier::Alt]);
    }

    #[test]
    fn collect_active_all_true() {
        let modifiers = Modifier::collect_active([
            (true, Modifier::Ctrl),
            (true, Modifier::Shift),
            (true, Modifier::Alt),
            (true, Modifier::Super),
        ]);
        assert_eq!(
            modifiers,
            vec![
                Modifier::Ctrl,
                Modifier::Shift,
                Modifier::Alt,
                Modifier::Super,
            ]
        );
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
        assert_eq!(modifiers, vec![Modifier::Shift]);
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
        let result = "Ctrl+@@@".into_hotkey();
        assert!(matches!(result, Err(ParseHotkeyError::UnknownToken(_))));
    }

    #[test]
    fn hotkey_input_from_string_reports_parse_error() {
        let result = String::from("Ctrl+@@@").into_hotkey();
        assert!(matches!(result, Err(ParseHotkeyError::UnknownToken(_))));
    }

    #[test]
    fn modifier_only_hotkey_with_alias_is_order_independent() {
        for input in ["Ctrl+Mod", "Mod+Ctrl"] {
            let hotkey = input.parse::<Hotkey>().unwrap();
            assert_eq!(hotkey.key(), Key::CONTROL_LEFT, "failed for {input}");
            assert_eq!(
                hotkey.modifiers(),
                &[Modifier::Alias(ModifierAlias::new("Mod"))],
                "failed for {input}"
            );
        }
    }

    #[test]
    fn alias_only_hotkey_returns_missing_key() {
        // An alias alone has no physical key to use as trigger.
        let result = "Mod".parse::<Hotkey>();
        assert!(matches!(result, Err(ParseHotkeyError::MissingKey)));
    }
}
