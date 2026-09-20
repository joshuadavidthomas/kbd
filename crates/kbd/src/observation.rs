//! Faithful keyboard observations and explicitly scoped immediate patterns.
//!
//! Logical characters are exact strings, not physical key positions or committed
//! text. No case folding, Unicode normalization, or layout inference is performed.
//! These programmatic types do not yet define a string parser or serde format.
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

use crate::hotkey::Hotkey;
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
        match self {
            Self::Physical(hotkey) => write!(f, "{hotkey}"),
            Self::Logical { key, modifiers } => write!(f, "logical:{modifiers:?}:{:?}", key.0),
        }
    }
}

impl PartialEq<Hotkey> for BindingPattern {
    fn eq(&self, hotkey: &Hotkey) -> bool {
        self.hotkey() == Some(*hotkey)
    }
}
