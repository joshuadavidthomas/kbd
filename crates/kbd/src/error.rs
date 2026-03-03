//! Core error types for hotkey operations.
//!
//! These cover domain-level errors: parsing failures, binding conflicts,
//! and layer operations. Platform-specific errors (backend init, device
//! access, permissions) belong in the runtime crate (`kbd-global`).

/// Core error type for hotkey operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A hotkey string could not be parsed.
    #[error("parse error: {0}")]
    Parse(#[from] crate::key::ParseHotkeyError),
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
}
