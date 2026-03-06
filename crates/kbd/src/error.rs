//! Error types for hotkey operations.
//!
//! [`ParseHotkeyError`] covers parsing failures for keys and hotkeys from strings.
//! [`Error`] covers domain-level errors: binding conflicts and layer operations.
//! Platform-specific errors (backend init, device access, permissions)
//! belong in the runtime crate (`kbd-global`).

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

/// Core error type for hotkey operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A hotkey string could not be parsed.
    #[error("parse error: {0}")]
    Parse(#[from] ParseHotkeyError),
    /// A binding for this hotkey is already registered.
    #[error("hotkey registration conflicts with an existing binding")]
    AlreadyRegistered,
    /// A layer with this name already exists.
    #[error("a layer with this name is already defined")]
    LayerAlreadyDefined,
    /// No layer with this name has been defined.
    #[error("no layer with this name has been defined")]
    LayerNotDefined,
    /// Tried to pop a layer but the stack is empty.
    #[error("no active layer to pop")]
    EmptyLayerStack,
    /// A modifier alias target must be a concrete modifier, not another alias.
    #[error("modifier alias target must be a concrete modifier (Ctrl, Shift, Alt, Super)")]
    InvalidAliasTarget,
    /// Defining or reassigning a modifier alias would make bindings ambiguous.
    #[error("modifier alias definition conflicts with an existing binding")]
    AliasConflict,
}
