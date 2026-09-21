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
//!     modifier_observation: None,
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
use crate::policy::LogicalMatchPolicy;
use crate::policy::PrimaryModifier;

/// A logical key identity, distinct from the physical [`Key`] domain.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LogicalKey(LogicalKeyValue);

impl AsRef<LogicalKeyValue> for LogicalKey {
    fn as_ref(&self) -> &LogicalKeyValue {
        &self.0
    }
}

impl From<LogicalKey> for LogicalKeyValue {
    fn from(key: LogicalKey) -> Self {
        key.0
    }
}

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

/// Modifier evidence in one domain. Unknown flags are not known-inactive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModifierState {
    active: ModifierSet,
    known: ModifierSet,
    extra_active: bool,
}

impl ModifierState {
    /// Record known flags; every reported active flag is necessarily known.
    #[must_use]
    pub const fn new(active: ModifierSet, known: ModifierSet) -> Self {
        Self {
            active,
            known: known.union(active),
            extra_active: false,
        }
    }

    /// Preserve active modifiers outside the representable vocabulary. These
    /// prevent exact matching rather than silently becoming an unmodified event.
    #[must_use]
    pub const fn with_extra_active(mut self, active: bool) -> Self {
        self.extra_active = active;
        self
    }

    /// Active representable modifiers.
    #[must_use]
    pub const fn active(self) -> ModifierSet {
        self.active
    }

    /// Modifiers whose state is known.
    #[must_use]
    pub const fn known(self) -> ModifierSet {
        self.known
    }

    /// Whether an unrepresentable modifier was reported active.
    #[must_use]
    pub const fn extra_active(self) -> bool {
        self.extra_active
    }

    fn matches(self, required: ModifierSet, consumed: Option<ModifierSet>) -> bool {
        if self.extra_active
            || required.intersection(self.known) != required
            || required.intersection(self.active) != required
        {
            return false;
        }
        let extras = ModifierSet::from_bits(self.active.bits() & !required.bits());
        consumed.map_or(extras.is_empty(), |consumed| {
            extras.intersection(consumed) == extras
        })
    }
}

/// Semantic layout evidence. Raw backend masks must first be translated using
/// the active keymap; neither right Alt nor a fixed XKB mask implies `AltGraph`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogicalModifiers {
    /// Semantic active and known modifiers, independent of physical flags.
    pub state: ModifierState,
    /// Known consumed modifiers. `None` means unavailable, forcing exact matching.
    pub consumed: Option<ModifierSet>,
}

/// Rich evidence supplied by adapters. Physical flags are never globally
/// reduced by logical consumed modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModifierObservation {
    /// Physical/backend-observed state.
    pub physical: ModifierState,
    /// Optional semantic layout enrichment.
    pub logical: Option<LogicalModifiers>,
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
    /// Optional richer evidence. Without it, legacy modifiers describe only
    /// the standard four flags plus explicitly active extended flags. Exact
    /// matching constrains known flags, never requires unknown flags, and
    /// rejects reported extras. Omitted extended flags remain unknown.
    pub modifier_observation: Option<ModifierObservation>,
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
            modifier_observation: None,
            transition,
        }
    }

    /// Project into the physical domain if its identity and active modifiers
    /// are representable. Unobserved flags remain outside this projection.
    #[must_use]
    pub fn physical_hotkey(&self) -> Option<Hotkey> {
        self.physical
            .filter(|_| !self.physical_modifiers().extra_active())
            .map(|key| Hotkey::with_modifiers(key, self.physical_modifiers().active()))
    }

    /// Physical modifier evidence, with a compatibility fallback for legacy input.
    #[must_use]
    pub fn physical_modifiers(&self) -> ModifierState {
        self.modifier_observation.map_or(
            ModifierState::new(self.modifiers, ModifierSet::STANDARD),
            |m| m.physical,
        )
    }

    /// Logical modifier evidence; no consumed information is inferred.
    #[must_use]
    pub fn logical_modifiers(&self) -> LogicalModifiers {
        self.modifier_observation
            .and_then(|m| m.logical)
            .unwrap_or(LogicalModifiers {
                state: self.physical_modifiers(),
                consumed: None,
            })
    }

    pub(crate) fn candidate_patterns(&self) -> Vec<BindingPattern> {
        let mut patterns = Vec::new();
        if let Some(hotkey) = self.physical_hotkey() {
            patterns.push(BindingPattern::Physical(hotkey));
        }
        if let Some(key) = &self.logical {
            let logical = self.logical_modifiers();
            let active = logical.state.active().bits();
            let removable = logical.consumed.unwrap_or(ModifierSet::NONE).bits() & active;
            let mut subset = removable;
            loop {
                patterns.push(BindingPattern::logical(
                    key.clone(),
                    ModifierSet::from_bits(active & !subset),
                ));
                if subset == 0 {
                    break;
                }
                subset = (subset - 1) & removable;
            }
        }
        patterns
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
        self.matches_with_policy(event, LogicalMatchPolicy::Exact)
    }

    /// Match once, with an explicit policy for logical consumed modifiers.
    #[must_use]
    pub fn matches_with_policy(
        &self,
        event: &KeyboardObservation,
        policy: LogicalMatchPolicy,
    ) -> bool {
        match self {
            Self::Physical(hotkey) => {
                event.physical == Some(hotkey.key())
                    && event
                        .physical_modifiers()
                        .matches(hotkey.modifier_set(), None)
            }
            Self::Logical { key, modifiers } => {
                let logical = event.logical_modifiers();
                let consumed = match policy {
                    LogicalMatchPolicy::Exact => None,
                    LogicalMatchPolicy::Consumed => logical.consumed,
                };
                event.logical.as_ref() == Some(key) && logical.state.matches(*modifiers, consumed)
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
                modifier_observation: None,
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

/// Configuration pattern retaining the `Primary` alias until an explicit
/// runtime policy resolves it. Observations and registered patterns never
/// contain Primary. Keep this value to re-register under a changed policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfiguredPattern {
    pattern: BindingPattern,
    primary: bool,
}

impl ConfiguredPattern {
    /// Resolve the alias before registration and conflict checking.
    #[must_use]
    pub fn resolve(&self, policy: PrimaryModifier) -> BindingPattern {
        let mut pattern = self.pattern.clone();
        if self.primary {
            let modifier = match policy {
                PrimaryModifier::Ctrl => Modifier::Ctrl,
                PrimaryModifier::Super => Modifier::Super,
            };
            match &mut pattern {
                BindingPattern::Physical(hotkey) => *hotkey = hotkey.modifier(modifier),
                BindingPattern::Logical { modifiers, .. } => *modifiers = modifiers.with(modifier),
            }
        }
        pattern
    }
}

impl std::str::FromStr for ConfiguredPattern {
    type Err = ParseHotkeyError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let tokens = split_quoted(input, '+')?;
        let (key, prefixes) = tokens.split_last().ok_or(ParseHotkeyError::Empty)?;
        let primary = prefixes
            .iter()
            .any(|token| token.eq_ignore_ascii_case("primary"));
        let input = prefixes
            .iter()
            .copied()
            .filter(|token| !token.eq_ignore_ascii_case("primary"))
            .chain(std::iter::once(*key))
            .collect::<Vec<_>>()
            .join("+");
        Ok(Self {
            pattern: input.parse()?,
            primary,
        })
    }
}

impl std::fmt::Display for ConfiguredPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.primary {
            f.write_str("Primary+")?;
        }
        self.pattern.fmt(f)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for ConfiguredPattern {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ConfiguredPattern {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
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
            let modifier = token.parse::<Modifier>()?;
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
