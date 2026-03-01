//! Core error types for keyboard shortcut operations.
//!
//! These cover domain-level errors: parsing failures, binding conflicts,
//! and layer operations. Platform-specific errors (backend init, device
//! access, permissions) belong in the runtime crate (`kbd-global`).

/// Core error type for keyboard shortcut operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("parse error: {0}")]
    Parse(#[from] crate::key::ParseHotkeyError),
    #[error("hotkey registration conflicts with an existing binding")]
    AlreadyRegistered,
    #[error("a layer with this name is already defined")]
    LayerAlreadyDefined,
    #[error("no layer with this name has been defined")]
    LayerNotDefined,
    #[error("no active layer to pop")]
    EmptyLayerStack,
}
