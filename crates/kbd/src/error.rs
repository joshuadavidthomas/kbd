//! Error types for hotkey operations.
//!
//! Errors are scoped to the operation that produces them:
//!
//! - [`ParseHotkeyError`] — parsing keys and hotkeys from strings
//! - [`RegisterError`] — binding registration (hotkeys, sequences, tap-hold)
//! - [`LayerError`] — layer definition and stack operations

/// Error returned when parsing a hotkey or key from a string fails.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ParseHotkeyError {
    /// The input string was empty.
    #[error("hotkey string is empty")]
    Empty,
    /// A segment between `+` separators was empty (e.g., `"Ctrl++A"`).
    #[error("hotkey contains an empty token")]
    EmptySegment,
    /// A token could not be recognized as a key or modifier.
    #[error("unknown hotkey token: {0}")]
    UnknownToken(String),
    /// The hotkey contained only modifiers with no trigger key.
    #[error("hotkey is missing a non-modifier key")]
    MissingKey,
    /// The hotkey contained more than one non-modifier key.
    #[error("hotkey has multiple non-modifier keys")]
    MultipleKeys,
}

/// Error returned when registering a binding fails.
///
/// Covers hotkey registration, sequence registration, and tap-hold
/// registration within the core dispatcher.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RegisterError {
    /// A hotkey string could not be parsed.
    #[error("parse error: {0}")]
    Parse(#[from] ParseHotkeyError),
    /// A binding for this hotkey is already registered.
    #[error("hotkey registration conflicts with an existing binding")]
    AlreadyRegistered,
}

/// Error returned when a layer operation fails.
///
/// Covers layer definition, push, pop, and toggle operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LayerError {
    /// A layer with this name already exists.
    #[error("a layer with this name is already defined")]
    AlreadyDefined,
    /// No layer with this name has been defined.
    #[error("no layer with this name has been defined")]
    NotDefined,
    /// Tried to pop a layer but the stack is empty.
    #[error("no active layer to pop")]
    EmptyStack,
}
