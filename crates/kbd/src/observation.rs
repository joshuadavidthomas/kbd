//! Faithful keyboard observations and explicitly scoped binding patterns.
//!
//! Logical characters are exact strings, not physical key positions or committed
//! text. No case folding, Unicode normalization, or layout inference is performed.
//! Patterns parse `Ctrl+physical:A`, `Ctrl+logical:"a"`, or `logical:Enter`.
//! Quoted logical values are exact character strings with JSON escapes; unquoted
//! logical values are named keys. Legacy unqualified hotkeys remain physical.
//! Display and optional serde use canonical domain-marked strings. The physical
//! [`Hotkey`] format is unchanged.
//!
//! ```
//! use kbd::action::Action;
//! use kbd::binding::BindingOptions;
//! use kbd::dispatcher::{Dispatcher, MatchResult};
//! use kbd::hotkey::ModifierSet;
//! use kbd::key::Key;
//! use kbd::key_state::KeyTransition;
//! use kbd::observation::{BindingPattern, KeyboardObservation, LogicalKeyValue};
//!
//! let mut dispatcher = Dispatcher::new();
//! let logical_a = LogicalKeyValue::Character("a".into());
//! dispatcher.register_pattern(
//!     BindingPattern::logical(logical_a.clone(), ModifierSet::NONE),
//!     Action::Suppress,
//!     BindingOptions::default(),
//! ).unwrap();
//! let event = KeyboardObservation {
//!     physical: Some(Key::Q), // for example, a layout that maps Q to "a"
//!     logical: Some(logical_a.into()),
//!     modifiers: ModifierSet::NONE,
//!     transition: KeyTransition::Press,
//! };
//! assert!(matches!(dispatcher.process_event(&event), MatchResult::Matched { .. }));
//! ```

/// Logical key vocabulary supplied by `keyboard-types`.
pub use keyboard_types::Key as LogicalKeyValue;
/// Named logical keys supplied by `keyboard-types`.
pub use keyboard_types::NamedKey;

use crate::error::ParseHotkeyError;
use crate::hotkey::Hotkey;
use crate::hotkey::Modifier;
use crate::hotkey::ModifierSet;
use crate::key::Key;
use crate::key_state::KeyTransition;

/// A logical key identity, distinct from the physical [`Key`] domain.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LogicalKey(pub LogicalKeyValue);

impl From<LogicalKeyValue> for LogicalKey {
    fn from(value: LogicalKeyValue) -> Self {
        Self(value)
    }
}

impl From<NamedKey> for LogicalKey {
    fn from(value: NamedKey) -> Self {
        Self(LogicalKeyValue::Named(value))
    }
}

/// One keyboard event. Neither identity is inferred from the other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardObservation {
    /// Physical position, when the producer knows it.
    pub physical: Option<Key>,
    /// Logical character string or named key, when known.
    pub logical: Option<LogicalKey>,
    /// Currently active modifiers, using the existing modifier model.
    pub modifiers: ModifierSet,
    /// Press, release, or OS repeat.
    pub transition: KeyTransition,
}

impl KeyboardObservation {
    /// Wrap a legacy physical hotkey without inventing a logical identity.
    #[must_use]
    pub const fn from_hotkey(hotkey: Hotkey, transition: KeyTransition) -> Self {
        Self {
            physical: Some(hotkey.key()),
            logical: None,
            modifiers: hotkey.modifier_set(),
            transition,
        }
    }

    /// Project an observation into the physical domain, if available.
    #[must_use]
    pub fn physical_hotkey(&self) -> Option<Hotkey> {
        self.physical
            .map(|key| Hotkey::with_modifiers(key, self.modifiers))
    }
}

/// An immediate binding explicitly targeting one identity domain.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BindingPattern {
    /// Match a physical position and exact modifiers (legacy semantics).
    Physical(Hotkey),
    /// Match an exact logical value and exact modifiers.
    Logical {
        /// Character string or named key.
        key: LogicalKey,
        /// Required modifiers.
        modifiers: ModifierSet,
    },
}

impl BindingPattern {
    /// Construct an exact logical pattern.
    #[must_use]
    pub fn logical(key: impl Into<LogicalKey>, modifiers: ModifierSet) -> Self {
        Self::Logical {
            key: key.into(),
            modifiers,
        }
    }

    /// Physical projection; logical patterns never fabricate physical positions.
    #[must_use]
    pub const fn hotkey(&self) -> Option<Hotkey> {
        match self {
            Self::Physical(hotkey) => Some(*hotkey),
            Self::Logical { .. } => None,
        }
    }

    /// Whether this pattern matches the supplied identities and modifiers.
    /// This predicate does not inspect transition, layers, or device context.
    /// The dispatcher applies those policies when processing the event.
    #[must_use]
    pub fn matches(&self, event: &KeyboardObservation) -> bool {
        match self {
            Self::Physical(hotkey) => event.physical_hotkey() == Some(*hotkey),
            Self::Logical { key, modifiers } => {
                event.logical.as_ref() == Some(key) && event.modifiers == *modifiers
            }
        }
    }

    /// A single-domain observation for pattern-specific introspection.
    pub(crate) fn observation(&self) -> KeyboardObservation {
        match self {
            Self::Physical(hotkey) => {
                KeyboardObservation::from_hotkey(*hotkey, KeyTransition::Press)
            }
            Self::Logical { key, modifiers } => KeyboardObservation {
                physical: None,
                logical: Some(key.clone()),
                modifiers: *modifiers,
                transition: KeyTransition::Press,
            },
        }
    }
}

impl From<Hotkey> for BindingPattern {
    fn from(hotkey: Hotkey) -> Self {
        Self::Physical(hotkey)
    }
}

impl std::fmt::Display for BindingPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let modifiers = match self {
            Self::Physical(hotkey) => hotkey.modifier_set(),
            Self::Logical { modifiers, .. } => *modifiers,
        };
        for modifier in modifiers {
            write!(f, "{modifier}+")?;
        }
        match self {
            Self::Physical(hotkey) => write!(f, "physical:{}", hotkey.key()),
            Self::Logical { key, .. } => match &key.0 {
                LogicalKeyValue::Character(text) => {
                    let quoted = serde_json::to_string(text).map_err(|_| std::fmt::Error)?;
                    write!(f, "logical:{quoted}")
                }
                LogicalKeyValue::Named(key) => write!(f, "logical:{key}"),
            },
        }
    }
}

impl std::str::FromStr for BindingPattern {
    type Err = ParseHotkeyError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if !input.contains(':') {
            return input.parse::<Hotkey>().map(Self::Physical);
        }
        let tokens = split_quoted(input, '+')?;
        let (key, modifiers) = tokens.split_last().ok_or(ParseHotkeyError::Empty)?;
        let mut modifier_set = ModifierSet::NONE;
        for token in modifiers {
            let modifier = token
                .parse::<Key>()
                .ok()
                .and_then(Modifier::from_key)
                .ok_or_else(|| ParseHotkeyError::UnknownToken((*token).into()))?;
            modifier_set = modifier_set.with(modifier);
        }
        if let Some(key) = key.strip_prefix("physical:") {
            return Ok(Self::Physical(Hotkey::with_modifiers(
                key.parse()?,
                modifier_set,
            )));
        }
        if let Some(key) = key.strip_prefix("logical:") {
            let key = key.trim();
            let logical = if key.starts_with('"') {
                LogicalKeyValue::Character(
                    serde_json::from_str::<String>(key)
                        .map_err(|_| ParseHotkeyError::InvalidQuotedKey)?,
                )
            } else {
                LogicalKeyValue::Named(
                    key.parse::<NamedKey>()
                        .map_err(|_| ParseHotkeyError::UnknownToken(key.into()))?,
                )
            };
            return Ok(Self::logical(logical, modifier_set));
        }
        Err(ParseHotkeyError::UnknownToken((*key).into()))
    }
}

/// Split only outside quoted character strings. The JSON decoder validates escapes.
pub(crate) fn split_quoted(input: &str, delimiter: char) -> Result<Vec<&str>, ParseHotkeyError> {
    let mut quoted = false;
    let mut escaped = false;
    let mut start = 0;
    let mut tokens = Vec::new();
    for (index, ch) in input.char_indices() {
        if escaped {
            escaped = false;
        } else if quoted && ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            quoted = !quoted;
        } else if !quoted && ch == delimiter {
            tokens.push(input[start..index].trim());
            start = index + ch.len_utf8();
        }
    }
    if quoted {
        return Err(ParseHotkeyError::InvalidQuotedKey);
    }
    tokens.push(input[start..].trim());
    if tokens.iter().any(|token| token.is_empty()) {
        return Err(if input.trim().is_empty() {
            ParseHotkeyError::Empty
        } else {
            ParseHotkeyError::EmptySegment
        });
    }
    Ok(tokens)
}

#[cfg(feature = "serde")]
impl serde::Serialize for BindingPattern {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for BindingPattern {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let input = String::deserialize(deserializer)?;
        input.parse().map_err(serde::de::Error::custom)
    }
}

impl PartialEq<Hotkey> for BindingPattern {
    fn eq(&self, hotkey: &Hotkey) -> bool {
        self.hotkey() == Some(*hotkey)
    }
}
